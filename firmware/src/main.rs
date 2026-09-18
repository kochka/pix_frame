#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]

use defmt::{error, info, warn};
use esp_bootloader_esp_idf::partitions::{self, PARTITION_TABLE_MAX_LEN};
use esp_hal::Blocking;
use esp_hal::clock::CpuClock;
use esp_hal::delay::Delay;
use esp_hal::gpio::{Input, InputConfig, Pull};
use esp_hal::main;
use esp_hal::rmt::Rmt;
use esp_hal::rng::Rng;
use esp_hal::rtc_cntl::Rtc;
use esp_hal::time::{Duration, Instant, Rate};
use esp_hal_smartled::{RmtSmartLeds, buffer_size, color_order};
use esp_println as _;
use esp_storage::FlashStorage;
use pix_frame_firmware::catalog::{Catalog, Entry, ScanEnd};
use pix_frame_firmware::clock::Clock;
use pix_frame_firmware::config::{Config, ConfigError};
use pix_frame_firmware::display::{Canvas, MatrixLayout, Wiring};
use pix_frame_firmware::player::{PlayError, Player};
use pix_frame_firmware::playlist::Playlist;
use pix_frame_firmware::screens;
use pix_frame_firmware::self_test;
use pix_frame_firmware::storage::{ByteSource, FlashPartition, NvsFlash};
use smart_leds::{RGB8, SmartLedsWrite, brightness};

// Physical description of the attached frame. This is the only place in the
// firmware that knows the matrix dimensions, wiring and channel order.
// See `doc/wiring.md`.
const LAYOUT: MatrixLayout = MatrixLayout {
    width: 16,
    height: 16,
    wiring: Wiring::Serpentine,
};
const LED_COUNT: usize = LAYOUT.led_count();

/// Global output brightness, 0..=255. Keep it modest: 256 WS2815 at full
/// white draw far more than a small supply can deliver.
const BRIGHTNESS: u8 = 60;

/// An animation loops until at least this much scheduled time has elapsed at
/// a loop boundary, then the next one starts.
const MIN_PLAY_TIME: Duration = Duration::from_millis(3_000);

/// Pause after a failed animation so a broken bundle does not spin the log.
const ERROR_BACKOFF: Duration = Duration::from_millis(500);

/// How long an information screen stays on before animations resume.
const INFO_SCREEN_TIME: Duration = Duration::from_millis(5_000);

/// Heap used by the NVS reader (and by the network stack later).
const HEAP_SIZE: usize = 64 * 1024;

/// Labels in `partitions.csv`.
const ANIMATIONS_PARTITION: &str = "animations";
const CONFIG_PARTITION: &str = "nvs";

type LedDriver<'d> =
    RmtSmartLeds<'d, { buffer_size::<RGB8>(LED_COUNT) }, Blocking, RGB8, color_order::Grb>;

#[panic_handler]
fn panic(panic_info: &core::panic::PanicInfo) -> ! {
    error!("{}", panic_info);
    loop {}
}

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

#[allow(
    clippy::large_stack_frames,
    reason = "it's not unusual to allocate larger buffers etc. in main"
)]
#[main]
fn main() -> ! {
    let hal_config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(hal_config);
    esp_alloc::heap_allocator!(size: HEAP_SIZE);

    // The WS2815 frame is wired to GPIO18; the RMT peripheral generates the
    // precisely timed signal the LEDs expect.
    let rmt_frequency = Rate::from_mhz(80);
    let rmt = Rmt::new(peripherals.RMT, rmt_frequency).unwrap();
    let mut led = LedDriver::new_with_memsize(
        esp_hal_smartled::WS2812_TIMING.with_reset_us(300),
        rmt.channel0,
        peripherals.GPIO18,
        2,
        rmt_frequency,
    )
    .unwrap();

    let delay = Delay::new();
    let mut pixels = [RGB8::new(0, 0, 0); LED_COUNT];
    // Self-test button: GPIO9 is the BOOT button of the Espressif ESP32-C6
    // dev boards, pulled up and low when pressed. Any GPIO with a button to
    // ground works the same way. See `doc/wiring.md` section 6.
    let mut button = Button::new(Input::new(
        peripherals.GPIO9,
        InputConfig::default().with_pull(Pull::Up),
    ));

    let mut flash = FlashStorage::new(peripherals.FLASH);
    let mut table_bytes = [0u8; PARTITION_TABLE_MAX_LEN];

    let config = match find_partition(&mut flash, &mut table_bytes, CONFIG_PARTITION) {
        Some((offset, len)) => load_config(&mut flash, offset, len),
        None => {
            warn!("partition '{}' not found, using defaults", CONFIG_PARTITION);
            Config::default()
        }
    };
    let clock = Clock::new(Rtc::new(peripherals.RTC_TIMER));
    let mut info_screens = screens::Rotation::default();

    let (offset, len) = match find_partition(&mut flash, &mut table_bytes, ANIMATIONS_PARTITION) {
        Some(window) => window,
        None => {
            error!(
                "partition '{}' not found, running the self-test",
                ANIMATIONS_PARTITION
            );
            self_test_forever(&mut led, &mut pixels, &delay);
        }
    };
    let mut source = match FlashPartition::new(&mut flash, offset, len) {
        Ok(source) => source,
        Err(err) => {
            error!("partition '{}' unusable: {}", ANIMATIONS_PARTITION, err);
            self_test_forever(&mut led, &mut pixels, &delay);
        }
    };

    let (catalog, end) = Catalog::scan(&mut source);
    info!(
        "{} animations in {} bytes at {=u32:#x} ({})",
        catalog.len(),
        source.len(),
        offset,
        end
    );
    if catalog.is_empty() {
        warn!("no animations found, running the self-test");
        self_test_forever(&mut led, &mut pixels, &delay);
    }
    if !matches!(end, ScanEnd::Terminator | ScanEnd::EndOfSource) {
        warn!("bundle indexing stopped early: {}", end);
    }

    let rng = Rng::new();
    let mut random = || rng.random();
    let mut playlist = Playlist::new(catalog.len(), &mut random);
    let mut since_info: u8 = 0;

    loop {
        if config.info_every > 0 && since_info >= config.info_every {
            since_info = 0;
            match info_screens.pick(&clock) {
                Some(screen) => {
                    info!("showing {} screen", screen);
                    let mut canvas = Canvas::new(&mut pixels, &LAYOUT);
                    if screens::render(screen, &mut canvas, &clock) {
                        show(&mut led, &pixels);
                        delay.delay(INFO_SCREEN_TIME);
                    }
                }
                None => info!("no information screen available yet"),
            }
        }

        let Some(entry) = playlist
            .next(&mut random)
            .and_then(|index| catalog.get(index))
        else {
            continue;
        };
        let header = &entry.header;
        info!(
            "playing animation at {=u32:#x}: {} frames, {} colors",
            entry.offset,
            header.frame_count(),
            header.palette_size()
        );
        match play(
            &mut led,
            &mut source,
            entry,
            &mut pixels,
            &delay,
            &mut button,
        ) {
            Ok(PlayEnd::Finished) => {}
            Ok(PlayEnd::Interrupted) => {
                info!("self-test button pressed");
                self_test::run(&mut led, &LAYOUT, &mut pixels, &delay);
            }
            Err(err) => {
                warn!("animation at {=u32:#x}: {}", entry.offset, err);
                delay.delay(ERROR_BACKOFF);
            }
        }
    }
}

/// Active-low push button that reports each press once.
struct Button {
    input: Input<'static>,
    held: bool,
}

impl Button {
    fn new(input: Input<'static>) -> Self {
        Self { input, held: false }
    }

    /// True on the first poll after the button went down; false until it is
    /// released and pressed again.
    fn take_press(&mut self) -> bool {
        let down = self.input.is_low();
        let pressed = down && !self.held;
        self.held = down;
        pressed
    }
}

/// Why [`play`] returned.
enum PlayEnd {
    /// The minimum play time was reached at a loop boundary.
    Finished,
    /// The self-test button was pressed.
    Interrupted,
}

/// Reads the device configuration from the NVS partition; any problem is
/// logged and leaves the defaults (no information screens, no network).
fn load_config(flash: &mut FlashStorage<'_>, offset: u32, len: u32) -> Config {
    let nvs = esp_nvs::Nvs::new(offset as usize, len as usize, NvsFlash(flash));
    let mut nvs = match nvs {
        Ok(nvs) => nvs,
        Err(err) => {
            warn!("config partition unreadable: {}", err);
            return Config::default();
        }
    };
    match Config::load(&mut nvs) {
        Ok(config) => {
            info!(
                "config: info screen every {} animations, wifi {}, location {}",
                config.info_every,
                config.wifi.as_ref().map_or("not set", |w| w.ssid.as_str()),
                config
                    .location
                    .as_ref()
                    .map_or("not set", |l| l.timezone.as_str())
            );
            config
        }
        Err(ConfigError::NotConfigured) => {
            info!("no configuration written yet, information screens disabled");
            Config::default()
        }
        Err(err) => {
            warn!("invalid configuration: {}", err);
            Config::default()
        }
    }
}

/// Offset and size of the partition labelled `label`, from the table in flash.
fn find_partition(
    flash: &mut FlashStorage<'_>,
    table_bytes: &mut [u8; PARTITION_TABLE_MAX_LEN],
    label: &str,
) -> Option<(u32, u32)> {
    let table = match partitions::read_partition_table(flash, table_bytes) {
        Ok(table) => table,
        Err(err) => {
            error!("cannot read the partition table: {}", err);
            return None;
        }
    };
    table
        .iter()
        .find(|entry| entry.label_as_str() == label)
        .map(|entry| (entry.offset(), entry.len()))
}

/// Plays one animation with deadline-based scheduling.
///
/// Frame `i + 1` is decoded while frame `i` is displayed, then sent exactly
/// when frame `i`'s duration has elapsed. Deadlines accumulate from the
/// first frame, so decoding and LED transmission time never add drift. A
/// late frame is shown as soon as it is ready; the schedule is not rebased.
/// The animation loops until [`MIN_PLAY_TIME`] is reached at a loop boundary
/// and the last frame keeps its full duration before returning. The button is
/// polled at every frame boundary.
#[allow(
    clippy::large_stack_frames,
    reason = "the player state is bounded by the display size (palette plus one packed frame)"
)]
fn play<S: ByteSource>(
    led: &mut LedDriver<'_>,
    source: &mut S,
    entry: &Entry,
    pixels: &mut [RGB8; LED_COUNT],
    delay: &Delay,
    button: &mut Button,
) -> Result<PlayEnd, PlayError> {
    let mut player = Player::<LED_COUNT>::open(source, entry, &LAYOUT)?;
    let mut deadline = Instant::now();
    let mut scheduled = Duration::ZERO;

    loop {
        let frame = player.next_frame(source, &LAYOUT, pixels)?;
        wait_until(deadline, delay);
        if button.take_press() {
            return Ok(PlayEnd::Interrupted);
        }
        show(led, pixels);

        let duration = Duration::from_millis(u64::from(frame.duration_ms));
        deadline += duration;
        scheduled += duration;

        if frame.is_last && scheduled >= MIN_PLAY_TIME {
            wait_until(deadline, delay);
            return Ok(PlayEnd::Finished);
        }
    }
}

fn wait_until(deadline: Instant, delay: &Delay) {
    let now = Instant::now();
    if now < deadline {
        delay.delay(deadline - now);
    }
}

fn show(led: &mut LedDriver<'_>, pixels: &[RGB8]) {
    if let Err(err) = led.write(brightness(pixels.iter().copied(), BRIGHTNESS)) {
        error!("LED write failed: {}", defmt::Debug2Format(&err));
    }
}

fn self_test_forever(led: &mut LedDriver<'_>, pixels: &mut [RGB8; LED_COUNT], delay: &Delay) -> ! {
    loop {
        self_test::run(led, &LAYOUT, pixels, delay);
    }
}
