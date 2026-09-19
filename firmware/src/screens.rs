//! Information screens shown between animations.
//!
//! Every screen has an availability condition checked each time one is due,
//! so an unconfigured or offline frame simply keeps playing animations.

use core::fmt::Write as _;

use heapless::String;
use smart_leds::RGB8;

use crate::clock::Clock;
use crate::display::Canvas;
use crate::font::{
    BIG_DIGIT_HEIGHT, BIG_PAIR_WIDTH, GLYPH_HEIGHT, draw_big_pair, draw_text_centered,
};
use crate::weather::{self, Condition, Observation};

const HOUR_COLOR: RGB8 = RGB8::new(255, 200, 60);
const MINUTE_COLOR: RGB8 = RGB8::new(90, 170, 255);
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
}

const ALL: [Screen; 2] = [Screen::Clock, Screen::Weather];

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
    }
}

/// Draws `screen` on a cleared canvas. Returns `false` when its data
/// disappeared between `pick` and now, leaving the canvas blank.
pub fn render(screen: Screen, canvas: &mut Canvas<'_>, clock: &Clock) -> bool {
    canvas.clear();
    match screen {
        Screen::Clock => match clock.local_time() {
            Some(time) => {
                render_clock(canvas, time.hour, time.minute);
                true
            }
            None => false,
        },
        Screen::Weather => match weather::current(clock.unix_time()) {
            Some(observation) => {
                render_weather(canvas, &observation);
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
pub fn render_weather(canvas: &mut Canvas<'_>, observation: &Observation) {
    let (sprite, primary, secondary) = icon(observation.condition);
    let gap = 1;
    let block = ICON_SIZE + gap + GLYPH_HEIGHT;
    let top = canvas.height().saturating_sub(block) / 2;
    let left = canvas.width().saturating_sub(ICON_SIZE) / 2;
    for (y, row) in sprite.iter().enumerate() {
        for (x, cell) in row.iter().enumerate() {
            let color = match cell {
                1 => primary,
                2 => secondary,
                _ => continue,
            };
            canvas.set(left + x, top + y, color);
        }
    }

    // Round tenths to the nearest degree, away from zero.
    let tenths = observation.temperature_dc;
    let celsius = (tenths + if tenths < 0 { -5 } else { 5 }) / 10;
    let mut text: String<8> = String::new();
    // Cannot fail: at most "-999°" is 6 bytes.
    let _ = write!(text, "{celsius}°");
    draw_text_centered(
        canvas,
        top + ICON_SIZE + gap,
        &text,
        temperature_color(celsius),
    );
}

/// Hours above minutes in large digits, centered.
pub fn render_clock(canvas: &mut Canvas<'_>, hour: u8, minute: u8) {
    let block_height = BIG_DIGIT_HEIGHT * 2 + 1;
    let x = canvas.width().saturating_sub(BIG_PAIR_WIDTH) / 2;
    let y = canvas.height().saturating_sub(block_height) / 2;
    draw_big_pair(canvas, x, y, hour, HOUR_COLOR);
    draw_big_pair(canvas, x, y + BIG_DIGIT_HEIGHT + 1, minute, MINUTE_COLOR);
}
