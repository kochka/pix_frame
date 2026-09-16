//! Logical-to-physical mapping of the LED matrix.
//!
//! This module deliberately has no hardware dependency so it can later be moved
//! to a host-testable crate. Everything that describes the physical frame
//! (dimensions, wiring pattern) is declared in `main.rs`, not here.

/// How consecutive rows are chained on the physical LED strip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wiring {
    /// Every row runs left to right.
    Progressive,
    /// Even rows run left to right, odd rows run right to left.
    Serpentine,
}

/// Logical dimensions of the frame and how its LEDs are chained.
///
/// Logical coordinates follow the PXA convention: origin at the top-left
/// pixel, `x` increasing to the right, `y` increasing downward.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MatrixLayout {
    pub width: usize,
    pub height: usize,
    pub wiring: Wiring,
}

impl MatrixLayout {
    /// Number of LEDs on the chain.
    pub const fn led_count(&self) -> usize {
        self.width * self.height
    }

    /// Physical LED index of the logical pixel `(x, y)`.
    ///
    /// Returns `None` when the coordinate is outside the matrix.
    pub fn physical_index(&self, x: usize, y: usize) -> Option<usize> {
        if x >= self.width || y >= self.height {
            return None;
        }

        let column = match self.wiring {
            Wiring::Progressive => x,
            Wiring::Serpentine if y.is_multiple_of(2) => x,
            Wiring::Serpentine => self.width - 1 - x,
        };

        Some(y * self.width + column)
    }
}
