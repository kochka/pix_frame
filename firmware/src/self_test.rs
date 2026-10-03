use core::fmt::Debug;

use esp_hal::{delay::Delay, rng::Rng};
use smart_leds::{RGB8, SmartLedsWrite};

use crate::color::random_vivid;
use crate::display::{Canvas, MatrixLayout};
use crate::font::{GLYPH_HEIGHT, draw_text_centered};
use crate::indoor::Reading;
use crate::screens::{WifiStatus, render_clock, render_indoor, render_weather, render_wifi};
use crate::weather::{Condition, Observation};

const TEST_BRIGHTNESS: u8 = 60;
const BLACK: RGB8 = RGB8::new(0, 0, 0);

/// Runs every hardware self-test once.
///
/// `pixels` must hold one entry per LED of `layout`; pixels outside the
/// layout are left untouched.
pub fn run<L, const N: usize>(
    led: &mut L,
    layout: &MatrixLayout,
    pixels: &mut [RGB8; N],
    delay: &Delay,
    wifi: WifiStatus,
) where
    L: SmartLedsWrite<Color = RGB8>,
    L::Error: Debug,
{
    test_single(led, pixels, delay);
    test_rgb(led, pixels, delay);
    test_single_vertical(led, layout, pixels, delay);
    test_random_blink(led, pixels, delay);
    test_random_fill(led, pixels, delay);
    test_text(led, layout, pixels, delay);
    test_wifi(led, layout, pixels, delay, wifi);
}

fn random_color() -> RGB8 {
    random_vivid(TEST_BRIGHTNESS)
}

fn clear(pixels: &mut [RGB8]) {
    pixels.fill(BLACK);
}

fn show<L>(led: &mut L, pixels: &[RGB8])
where
    L: SmartLedsWrite<Color = RGB8>,
    L::Error: Debug,
{
    led.write(pixels.iter().copied()).unwrap();
}

/// Lights every LED in physical chain order.
fn test_single<L>(led: &mut L, pixels: &mut [RGB8], delay: &Delay)
where
    L: SmartLedsWrite<Color = RGB8>,
    L::Error: Debug,
{
    let color = random_color();
    clear(pixels);

    for index in 0..pixels.len() {
        pixels[index] = color;
        show(led, pixels);
        pixels[index] = BLACK;
        delay.delay_millis(10);
    }
}

/// Lights every LED column by column in logical order, which reveals whether
/// the configured wiring matches the physical frame.
fn test_single_vertical<L>(led: &mut L, layout: &MatrixLayout, pixels: &mut [RGB8], delay: &Delay)
where
    L: SmartLedsWrite<Color = RGB8>,
    L::Error: Debug,
{
    let color = random_color();
    clear(pixels);

    for x in 0..layout.width {
        for y in 0..layout.height {
            let Some(index) = layout.physical_index(x, y) else {
                continue;
            };
            if index >= pixels.len() {
                continue;
            }

            pixels[index] = color;
            show(led, pixels);
            pixels[index] = BLACK;
            delay.delay_millis(10);
        }
    }
}

fn test_rgb<L>(led: &mut L, pixels: &mut [RGB8], delay: &Delay)
where
    L: SmartLedsWrite<Color = RGB8>,
    L::Error: Debug,
{
    for color in [
        RGB8::new(255, 0, 0),
        RGB8::new(0, 255, 0),
        RGB8::new(0, 0, 255),
    ] {
        pixels.fill(color);
        show(led, pixels);
        delay.delay_millis(500);
    }
}

fn test_random_blink<L>(led: &mut L, pixels: &mut [RGB8], delay: &Delay)
where
    L: SmartLedsWrite<Color = RGB8>,
    L::Error: Debug,
{
    for _ in 0..5 {
        pixels.fill(random_color());
        show(led, pixels);
        delay.delay_millis(200);

        clear(pixels);
        show(led, pixels);
        delay.delay_millis(200);
    }
}

fn test_random_fill<L, const N: usize>(led: &mut L, pixels: &mut [RGB8; N], delay: &Delay)
where
    L: SmartLedsWrite<Color = RGB8>,
    L::Error: Debug,
{
    const {
        assert!(
            N <= u16::MAX as usize + 1,
            "random fill indexes LEDs with u16"
        );
    }

    let rng = Rng::new();
    let mut available: [u16; N] = core::array::from_fn(|index| index as u16);
    let mut remaining = N;

    clear(pixels);

    while remaining > 0 {
        let random_position = rng.random() as usize % remaining;
        let pixel_index = usize::from(available[random_position]);

        available.swap(random_position, remaining - 1);
        remaining -= 1;

        pixels[pixel_index] = random_color();
        show(led, pixels);
        delay.delay_millis(10);
    }
}

/// Renders text and the clock layout through the fonts and the logical mapping.
fn test_text<L>(led: &mut L, layout: &MatrixLayout, pixels: &mut [RGB8], delay: &Delay)
where
    L: SmartLedsWrite<Color = RGB8>,
    L::Error: Debug,
{
    let color = random_color();
    let mut canvas = Canvas::new(pixels, layout);
    canvas.clear();
    let y = layout.height.saturating_sub(GLYPH_HEIGHT) / 2;
    draw_text_centered(&mut canvas, y, "TEST", color);
    show(led, pixels);
    delay.delay_millis(1500);

    clear(pixels);

    let mut canvas = Canvas::new(pixels, layout);
    render_clock(&mut canvas, 12, 34);
    show(led, pixels);
    delay.delay_millis(1500);

    // Every weather icon in turn, with temperatures spanning the color scale.
    let temperatures_dc = [305, 215, 174, 5, -35, -120, 280];
    for (condition, temperature_dc) in Condition::ALL.into_iter().zip(temperatures_dc) {
        clear(pixels);

        let mut canvas = Canvas::new(pixels, layout);
        render_weather(
            &mut canvas,
            &Observation {
                temperature_dc,
                condition,
                utc_offset: 0,
            },
        );
        show(led, pixels);
        delay.delay_millis(600);
    }

    clear(pixels);
    let mut canvas = Canvas::new(pixels, layout);
    render_indoor(
        &mut canvas,
        &Reading {
            temperature_dc: 215,
            humidity: 48,
        },
    );
    show(led, pixels);
    delay.delay_millis(1500);
}

/// Shows the Wi-Fi connection status.
fn test_wifi<L>(
    led: &mut L,
    layout: &MatrixLayout,
    pixels: &mut [RGB8],
    delay: &Delay,
    wifi: WifiStatus,
) where
    L: SmartLedsWrite<Color = RGB8>,
    L::Error: Debug,
{
    let mut canvas = Canvas::new(pixels, layout);
    render_wifi(&mut canvas, wifi);
    show(led, pixels);
    delay.delay_millis(2000);
    clear(pixels);
    show(led, pixels);
}
