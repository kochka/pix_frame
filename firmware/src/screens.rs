//! Information screens shown between animations.
//!
//! Every screen has an availability condition checked each time one is due,
//! so an unconfigured or offline frame simply keeps playing animations.

use smart_leds::RGB8;

use crate::clock::Clock;
use crate::display::Canvas;
use crate::font::{BIG_DIGIT_HEIGHT, BIG_PAIR_WIDTH, draw_big_pair};

const HOUR_COLOR: RGB8 = RGB8::new(255, 200, 60);
const MINUTE_COLOR: RGB8 = RGB8::new(90, 170, 255);

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
