//! Logical-to-physical mapping of the LED matrix.
//!
//! This module deliberately has no hardware dependency so it can later be moved
//! to a host-testable crate. Everything that describes the physical frame
//! (dimensions, wiring pattern) is declared in `main.rs`, not here.

use smart_leds::RGB8;

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

/// Logical drawing surface over the physical pixel buffer.
pub struct Canvas<'a> {
    pixels: &'a mut [RGB8],
    layout: &'a MatrixLayout,
}

impl<'a> Canvas<'a> {
    pub fn new(pixels: &'a mut [RGB8], layout: &'a MatrixLayout) -> Self {
        Self { pixels, layout }
    }

    pub const fn width(&self) -> usize {
        self.layout.width
    }

    pub const fn height(&self) -> usize {
        self.layout.height
    }

    pub fn clear(&mut self) {
        self.pixels.fill(RGB8::new(0, 0, 0));
    }

    /// Sets the logical pixel `(x, y)`; out-of-range coordinates are ignored.
    pub fn set(&mut self, x: usize, y: usize, color: RGB8) {
        if let Some(index) = self.layout.physical_index(x, y)
            && let Some(pixel) = self.pixels.get_mut(index)
        {
            *pixel = color;
        }
    }
}
