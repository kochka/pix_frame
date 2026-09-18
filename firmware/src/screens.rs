//! Information screens shown between animations.
//!
//! Every screen has an availability condition checked each time one is due,
//! so an unconfigured or offline frame simply keeps playing animations.

use smart_leds::RGB8;

use crate::clock::Clock;
use crate::display::Canvas;
use crate::font::{
    BIG_DIGIT_HEIGHT, BIG_PAIR_WIDTH, GLYPH_HEIGHT, draw_big_pair, draw_text_centered,
};

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
}

const ALL: [Screen; 1] = [Screen::Clock];

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
    }
}

/// Hours above minutes in large digits, centered.
pub fn render_clock(canvas: &mut Canvas<'_>, hour: u8, minute: u8) {
    let block_height = BIG_DIGIT_HEIGHT * 2 + 1;
    let x = canvas.width().saturating_sub(BIG_PAIR_WIDTH) / 2;
    let y = canvas.height().saturating_sub(block_height) / 2;
    draw_big_pair(canvas, x, y, hour, HOUR_COLOR);
    draw_big_pair(canvas, x, y + BIG_DIGIT_HEIGHT + 1, minute, MINUTE_COLOR);
}
