use crate::MAX_PALETTE_SIZE;

/// Storage depth of packed palette indices, derived from the palette size.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum BitsPerPixel {
    One = 1,
    Two = 2,
    Four = 4,
    Eight = 8,
}

impl BitsPerPixel {
    /// Selects the depth for a palette, as defined by PXA v1 section 9.
    ///
    /// Returns `None` when the palette size is not within `1..=256`.
    pub const fn from_palette_size(palette_size: u16) -> Option<Self> {
        match palette_size {
            0 => None,
            1..=2 => Some(Self::One),
            3..=4 => Some(Self::Two),
            5..=16 => Some(Self::Four),
            17..=MAX_PALETTE_SIZE => Some(Self::Eight),
            _ => None,
        }
    }

    /// Number of bits used by one pixel.
    pub const fn bits(self) -> u8 {
        self as u8
    }

    /// Number of pixels stored in one byte.
    pub const fn pixels_per_byte(self) -> u8 {
        8 / self.bits()
    }

    /// Mask selecting one pixel value once shifted to the low bits.
    pub const fn mask(self) -> u8 {
        match self {
            Self::Eight => 0xFF,
            _ => (1u8 << self.bits()) - 1,
        }
    }

    /// Number of payload bytes needed for `pixel_count` pixels.
    ///
    /// Returns `None` when the result does not fit in the `u32` payload size
    /// field. For all dimensions representable in a PXA header the result
    /// always fits, but callers must still handle `None`.
    pub const fn packed_len(self, pixel_count: u32) -> Option<u32> {
        let bits = pixel_count as u64 * self.bits() as u64;
        let bytes = bits.div_ceil(8);
        if bytes > u32::MAX as u64 {
            None
        } else {
            Some(bytes as u32)
        }
    }
}
