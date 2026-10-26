//! Information screens shown between animations.
//!
//! Every screen has an availability condition checked each time one is due,
//! so an unconfigured or offline frame simply keeps playing animations.

use core::fmt::Write as _;

use heapless::String;
use smart_leds::RGB8;

use crate::clock::Clock;
use crate::config::{Style, TemperatureUnit};
use crate::display::Canvas;
use crate::font::{
    BIG_DIGIT_GAP, BIG_DIGIT_HEIGHT, BIG_DIGIT_WIDTH, BIG_PAIR_WIDTH, GLYPH_HEIGHT, draw_big_pair,
    draw_text_centered, text_width,
};
use crate::indoor::{self, Reading};
use crate::weather::{self, Condition, Observation};

const SEPARATOR_COLOR: RGB8 = RGB8::new(200, 200, 200);
const LABEL_COLOR: RGB8 = RGB8::new(200, 200, 200);
const OK_COLOR: RGB8 = RGB8::new(0, 200, 40);
const KO_COLOR: RGB8 = RGB8::new(220, 20, 20);
const PENDING_COLOR: RGB8 = RGB8::new(230, 200, 0);
const OFF_COLOR: RGB8 = RGB8::new(90, 90, 90);

/// Wi-Fi connection state, as published by the network tasks.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, defmt::Format)]
pub enum WifiStatus {
    /// No `[wifi]` section in the configuration.
    Off = 0,
    Connecting = 1,
    Connected = 2,
    /// The last connection attempt failed; retries continue.
    Failed = 3,
}

impl WifiStatus {
    pub fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::Connecting,
            2 => Self::Connected,
            3 => Self::Failed,
            _ => Self::Off,
        }
    }
}

/// "WIFI" above the status: OK in green, KO in red, OFF when unconfigured.
pub fn render_wifi(canvas: &mut Canvas<'_>, status: WifiStatus) {
    canvas.clear();
    let (text, color) = match status {
        WifiStatus::Off => ("OFF", OFF_COLOR),
        WifiStatus::Connecting => ("...", PENDING_COLOR),
        WifiStatus::Connected => ("OK", OK_COLOR),
        WifiStatus::Failed => ("KO", KO_COLOR),
    };
    let gap = 3;
    let top = canvas.height().saturating_sub(GLYPH_HEIGHT * 2 + gap) / 2;
    draw_text_centered(canvas, top, "WIFI", LABEL_COLOR);
    draw_text_centered(canvas, top + GLYPH_HEIGHT + gap, text, color);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, defmt::Format)]
pub enum Screen {
    Clock,
    Weather,
    Temperature,
    Humidity,
}

const ALL: [Screen; 4] = [
    Screen::Clock,
    Screen::Weather,
    Screen::Temperature,
    Screen::Humidity,
];

/// Round-robin over the screens that are currently available.
#[derive(Default)]
pub struct Rotation {
    next: usize,
}

impl Rotation {
    /// Next available screen after the last one shown, if any.
    pub fn pick(&mut self, clock: &Clock) -> Option<Screen> {
        for step in 0..ALL.len() {
            let index = (self.next + step) % ALL.len();
            if available(ALL[index], clock) {
                self.next = index + 1;
                return Some(ALL[index]);
            }
        }
        None
    }
}

fn available(screen: Screen, clock: &Clock) -> bool {
    match screen {
        Screen::Clock => clock.is_synced(),
        Screen::Weather => weather::current(clock.unix_time()).is_some(),
        Screen::Temperature | Screen::Humidity => indoor::current().is_some(),
    }
}

/// Draws `screen` on a cleared canvas. Returns `false` when its data
/// disappeared between `pick` and now, leaving the canvas blank.
pub fn render(screen: Screen, canvas: &mut Canvas<'_>, clock: &Clock, style: &Style) -> bool {
    canvas.clear();
    match screen {
        Screen::Clock => match clock.local_time() {
            Some(time) => {
                render_clock(canvas, style, time.hour, time.minute, time.second % 2 == 0);
                true
            }
            None => false,
        },
        Screen::Weather => match weather::current(clock.unix_time()) {
            Some(observation) => {
                render_weather(canvas, style, &observation);
                true
            }
            None => false,
        },
        Screen::Temperature => match indoor::current() {
            Some(reading) => {
                render_temperature(canvas, style, &reading);
                true
            }
            None => false,
        },
        Screen::Humidity => match indoor::current() {
            Some(reading) => {
                render_humidity(canvas, style, &reading);
                true
            }
            None => false,
        },
    }
}

const SUN: RGB8 = RGB8::new(255, 190, 0);
const CLOUD: RGB8 = RGB8::new(170, 170, 180);
const DARK_CLOUD: RGB8 = RGB8::new(110, 110, 125);
const RAIN: RGB8 = RGB8::new(60, 120, 255);
const SNOW: RGB8 = RGB8::new(230, 240, 255);
const LIGHTNING: RGB8 = RGB8::new(255, 230, 40);
const FOG: RGB8 = RGB8::new(140, 140, 150);

const ICON_SIZE: usize = 8;
type Icon = [[u8; ICON_SIZE]; ICON_SIZE];

/// Sprite cells: 0 blank, 1 primary color, 2 secondary color.
fn icon(condition: Condition) -> (Icon, RGB8, RGB8) {
    match condition {
        Condition::Clear => (
            [
                [0, 0, 0, 1, 1, 0, 0, 0],
                [0, 1, 0, 0, 0, 0, 1, 0],
                [0, 0, 1, 1, 1, 1, 0, 0],
                [1, 0, 1, 1, 1, 1, 0, 1],
                [1, 0, 1, 1, 1, 1, 0, 1],
                [0, 0, 1, 1, 1, 1, 0, 0],
                [0, 1, 0, 0, 0, 0, 1, 0],
                [0, 0, 0, 1, 1, 0, 0, 0],
            ],
            SUN,
            SUN,
        ),
        Condition::PartlyCloudy => (
            [
                [0, 0, 1, 1, 0, 0, 0, 0],
                [0, 1, 1, 1, 1, 0, 0, 0],
                [0, 1, 1, 2, 2, 0, 0, 0],
                [0, 0, 2, 2, 2, 2, 2, 0],
                [0, 2, 2, 2, 2, 2, 2, 2],
                [2, 2, 2, 2, 2, 2, 2, 2],
                [2, 2, 2, 2, 2, 2, 2, 2],
                [0, 2, 2, 2, 2, 2, 2, 0],
            ],
            SUN,
            CLOUD,
        ),
        Condition::Cloudy => (
            [
                [0, 0, 0, 0, 0, 0, 0, 0],
                [0, 0, 1, 1, 1, 0, 0, 0],
                [0, 1, 1, 1, 1, 1, 1, 0],
                [1, 1, 1, 1, 1, 1, 1, 1],
                [1, 1, 1, 1, 1, 1, 1, 1],
                [0, 1, 1, 1, 1, 1, 1, 0],
                [0, 0, 0, 0, 0, 0, 0, 0],
                [0, 0, 0, 0, 0, 0, 0, 0],
            ],
            CLOUD,
            CLOUD,
        ),
        Condition::Fog => (
            [
                [0, 0, 0, 0, 0, 0, 0, 0],
                [1, 1, 1, 1, 1, 1, 0, 0],
                [0, 0, 0, 0, 0, 0, 0, 0],
                [0, 1, 1, 1, 1, 1, 1, 1],
                [0, 0, 0, 0, 0, 0, 0, 0],
                [1, 1, 1, 1, 1, 1, 0, 0],
                [0, 0, 0, 0, 0, 0, 0, 0],
                [0, 0, 1, 1, 1, 1, 1, 1],
            ],
            FOG,
            FOG,
        ),
        Condition::Rain => (
            [
                [0, 0, 1, 1, 1, 0, 0, 0],
                [0, 1, 1, 1, 1, 1, 1, 0],
                [1, 1, 1, 1, 1, 1, 1, 1],
                [0, 1, 1, 1, 1, 1, 1, 0],
                [0, 0, 0, 0, 0, 0, 0, 0],
                [0, 2, 0, 0, 2, 0, 0, 2],
                [2, 0, 0, 2, 0, 0, 2, 0],
                [0, 0, 2, 0, 0, 2, 0, 0],
            ],
            DARK_CLOUD,
            RAIN,
        ),
        Condition::Snow => (
            [
                [0, 0, 1, 1, 1, 0, 0, 0],
                [0, 1, 1, 1, 1, 1, 1, 0],
                [1, 1, 1, 1, 1, 1, 1, 1],
                [0, 1, 1, 1, 1, 1, 1, 0],
                [0, 0, 0, 0, 0, 0, 0, 0],
                [0, 2, 0, 0, 2, 0, 0, 2],
                [0, 0, 0, 0, 0, 0, 0, 0],
                [2, 0, 0, 2, 0, 0, 2, 0],
            ],
            CLOUD,
            SNOW,
        ),
        Condition::Thunderstorm => (
            [
                [0, 0, 1, 1, 1, 0, 0, 0],
                [0, 1, 1, 1, 1, 1, 1, 0],
                [1, 1, 1, 1, 1, 1, 1, 1],
                [0, 1, 1, 1, 1, 1, 1, 0],
                [0, 0, 0, 0, 2, 0, 0, 0],
                [0, 0, 0, 2, 2, 0, 0, 0],
                [0, 0, 0, 0, 2, 0, 0, 0],
                [0, 0, 0, 2, 0, 0, 0, 0],
            ],
            DARK_CLOUD,
            LIGHTNING,
        ),
    }
}

/// Temperature color: blue when freezing, white when mild, red when hot.
fn temperature_color(celsius: i32) -> RGB8 {
    match celsius {
        i32::MIN..=0 => RGB8::new(80, 140, 255),
        1..=10 => RGB8::new(120, 200, 255),
        11..=20 => RGB8::new(230, 230, 230),
        21..=27 => RGB8::new(255, 170, 60),
        _ => RGB8::new(255, 60, 40),
    }
}

/// Condition icon on top, the temperature in whole degrees below.
pub fn render_weather(canvas: &mut Canvas<'_>, style: &Style, observation: &Observation) {
    let (sprite, primary, secondary) = icon(observation.condition);
    let celsius = round_to_degree(observation.temperature_dc);
    let degrees = round_to_degree(in_unit(observation.temperature_dc, style.temperature_unit));
    let mut text: String<16> = String::new();
    // Cannot fail: the widest i32 gives "-214748364°", 12 bytes.
    let _ = write!(text, "{degrees}°");
    draw_icon_over_text(
        canvas,
        &sprite,
        (primary, secondary),
        &text,
        temperature_color(celsius),
    );
}

/// Rounds tenths of a degree to the nearest degree, away from zero.
fn round_to_degree(tenths: i32) -> i32 {
    (tenths + if tenths < 0 { -5 } else { 5 }) / 10
}

/// Converts tenths of a degree Celsius to tenths in `unit`, rounded.
fn in_unit(celsius_dc: i32, unit: TemperatureUnit) -> i32 {
    match unit {
        TemperatureUnit::Celsius => celsius_dc,
        TemperatureUnit::Fahrenheit => {
            let scaled = celsius_dc.saturating_mul(18);
            (scaled + if scaled < 0 { -5 } else { 5 }) / 10 + 320
        }
    }
}

/// Draws a sprite above `text`, both horizontally centered, the block
/// vertically centered.
fn draw_icon_over_text<const W: usize>(
    canvas: &mut Canvas<'_>,
    sprite: &[[u8; W]],
    colors: (RGB8, RGB8),
    text: &str,
    text_color: RGB8,
) {
    let gap = 1;
    let block = sprite.len() + gap + GLYPH_HEIGHT;
    let top = canvas.height().saturating_sub(block) / 2;
    let left = canvas.width().saturating_sub(W) / 2;
    draw_sprite(canvas, left, top, sprite, colors.0, colors.1);
    draw_text_centered(canvas, top + sprite.len() + gap, text, text_color);
}

const GLASS: RGB8 = RGB8::new(150, 150, 160);
const MERCURY: RGB8 = RGB8::new(230, 30, 20);
const INDOOR_TEXT_COLOR: RGB8 = RGB8::new(230, 215, 190);
const HUMIDITY_COLOR: RGB8 = RGB8::new(60, 160, 255);
const DROP_HIGHLIGHT: RGB8 = RGB8::new(170, 220, 255);

/// Thermometer with a scale, cells as in [`icon`]: 1 glass and
/// graduations, 2 mercury.
const THERMOMETER: [[u8; 7]; ICON_SIZE] = [
    [0, 0, 1, 0, 0, 0, 0],
    [0, 1, 0, 1, 0, 1, 1],
    [0, 1, 0, 1, 0, 0, 0],
    [0, 1, 2, 1, 0, 1, 1],
    [0, 1, 2, 1, 0, 0, 0],
    [1, 2, 2, 2, 1, 0, 0],
    [1, 2, 2, 2, 1, 0, 0],
    [0, 1, 1, 1, 0, 0, 0],
];

/// Water drop, cells as in [`icon`]: 1 water, 2 highlight.
const DROP: [[u8; 7]; ICON_SIZE] = [
    [0, 0, 0, 1, 0, 0, 0],
    [0, 0, 0, 1, 0, 0, 0],
    [0, 0, 1, 1, 1, 0, 0],
    [0, 1, 1, 1, 1, 1, 0],
    [1, 2, 1, 1, 1, 1, 1],
    [1, 2, 1, 1, 1, 1, 1],
    [0, 1, 1, 1, 1, 1, 0],
    [0, 0, 1, 1, 1, 0, 0],
];

fn draw_sprite<const W: usize>(
    canvas: &mut Canvas<'_>,
    x: usize,
    y: usize,
    sprite: &[[u8; W]],
    primary: RGB8,
    secondary: RGB8,
) {
    for (dy, row) in sprite.iter().enumerate() {
        for (dx, cell) in row.iter().enumerate() {
            let color = match cell {
                1 => primary,
                2 => secondary,
                _ => continue,
            };
            canvas.set(x + dx, y + dy, color);
        }
    }
}

/// A thermometer above the indoor temperature in tenths of a degree, or whole
/// degrees when the tenths do not fit (100 °F and above, -10 and below).
pub fn render_temperature(canvas: &mut Canvas<'_>, style: &Style, reading: &Reading) {
    let tenths = in_unit(reading.temperature_dc, style.temperature_unit);
    let sign = if tenths < 0 { "-" } else { "" };
    let magnitude = tenths.unsigned_abs();
    let mut text: String<16> = String::new();
    // Cannot fail: the widest i32 gives "-214748364.8°", 14 bytes.
    let _ = write!(text, "{sign}{}.{}°", magnitude / 10, magnitude % 10);
    if text_width(&text) > canvas.width() {
        text.clear();
        let _ = write!(text, "{}°", round_to_degree(tenths));
    }
    draw_icon_over_text(
        canvas,
        &THERMOMETER,
        (GLASS, MERCURY),
        &text,
        INDOOR_TEXT_COLOR,
    );
}

/// A drop above the indoor relative humidity.
pub fn render_humidity(canvas: &mut Canvas<'_>, _style: &Style, reading: &Reading) {
    let mut text: String<8> = String::new();
    // Cannot fail: at most "255%" is 4 bytes.
    let _ = write!(text, "{}%", reading.humidity);
    draw_icon_over_text(
        canvas,
        &DROP,
        (HUMIDITY_COLOR, DROP_HIGHLIGHT),
        &text,
        HUMIDITY_COLOR,
    );
}

/// Hours above minutes in large digits, centered. The separator is a
/// square in the free space between the four digits, so it touches none.
pub fn render_clock(canvas: &mut Canvas<'_>, style: &Style, hour: u8, minute: u8, separator: bool) {
    let gap = BIG_DIGIT_GAP;
    let block_height = BIG_DIGIT_HEIGHT * 2 + gap;
    let x = canvas.width().saturating_sub(BIG_PAIR_WIDTH) / 2;
    let y = canvas.height().saturating_sub(block_height) / 2;
    draw_big_pair(canvas, x, y, hour, style.hour_color);
    draw_big_pair(
        canvas,
        x,
        y + BIG_DIGIT_HEIGHT + gap,
        minute,
        style.minute_color,
    );
    if separator {
        for dy in 0..gap {
            for dx in 0..BIG_DIGIT_GAP {
                canvas.set(
                    x + BIG_DIGIT_WIDTH + dx,
                    y + BIG_DIGIT_HEIGHT + dy,
                    SEPARATOR_COLOR,
                );
            }
        }
    }
}
