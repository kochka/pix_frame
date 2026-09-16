#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]

use defmt::error;
use esp_hal::Blocking;
use esp_hal::clock::CpuClock;
use esp_hal::delay::Delay;
use esp_hal::main;
use esp_hal::rmt::Rmt;
use esp_hal::time::Rate;
use esp_hal_smartled::{RmtSmartLeds, buffer_size, color_order};
use esp_println as _;
use pix_frame_firmware::display::{MatrixLayout, Wiring};
use pix_frame_firmware::self_test;
use smart_leds::RGB8;

// Physical description of the attached frame. This is the only place in the
// firmware that knows the matrix dimensions, wiring and channel order.
// See `doc/wiring.md`.
const LAYOUT: MatrixLayout = MatrixLayout {
    width: 16,
    height: 16,
    wiring: Wiring::Serpentine,
};
const LED_COUNT: usize = LAYOUT.led_count();

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
    // generator version: 1.3.0
    // generator parameters: --chip esp32c6 -o unstable-hal -o zed -o defmt -o stable-x86_64-unknown-linux-gnu

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    // Le cadre de LED WS2815 est relie au GPIO18.
    // Le peripherique RMT genere le signal precis attendu par les LED.
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

    loop {
        self_test::run(&mut led, &LAYOUT, &mut pixels, &delay);
    }

    // for inspiration have a look at the examples at https://github.com/esp-rs/esp-hal/tree/esp-hal-v1.1.0/examples
}
