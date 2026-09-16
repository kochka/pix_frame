use crate::{BitsPerPixel, FrameError};

/// Packs palette indices MSB-first into `out`, as described in section 12.
///
/// `out` must hold at least `bpp.packed_len(indices.len())` bytes; the used
/// prefix is fully overwritten and padding bits are written as zero. Every
/// index must be lower than `palette_size`.
///
/// Returns the number of bytes written.
pub fn pack(
    indices: &[u8],
    bpp: BitsPerPixel,
    palette_size: u16,
    out: &mut [u8],
) -> Result<usize, FrameError> {
    let pixel_count = u32::try_from(indices.len()).map_err(|_| FrameError::PixelCountMismatch {
        expected: u32::MAX,
        actual: indices.len(),
    })?;
    let len = bpp
        .packed_len(pixel_count)
        .and_then(|len| usize::try_from(len).ok())
        .ok_or(FrameError::PixelCountMismatch {
            expected: u32::MAX,
            actual: indices.len(),
        })?;
    let out = out.get_mut(..len).ok_or(FrameError::TruncatedPayload {
        expected: pixel_count,
        actual: 0,
    })?;
    out.fill(0);

    let per_byte = usize::from(bpp.pixels_per_byte());
    let bits = bpp.bits();
    for (pixel, &index) in indices.iter().enumerate() {
        if u16::from(index) >= palette_size {
            return Err(FrameError::InvalidPaletteIndex {
                pixel: pixel as u32,
                index,
            });
        }
        let slot = (pixel % per_byte) as u8;
        let shift = 8 - bits * (slot + 1);
        out[pixel / per_byte] |= index << shift;
    }
    Ok(len)
}

/// Validates a `RAW_INDEXED` payload without producing output.
///
/// Checks the payload length, every palette index and the padding bits.
pub fn validate_payload(
    payload: &[u8],
    bpp: BitsPerPixel,
    pixel_count: u32,
    palette_size: u16,
) -> Result<(), FrameError> {
    let expected = bpp
        .packed_len(pixel_count)
        .ok_or(FrameError::PayloadSizeMismatch {
            expected: u32::MAX,
            actual: 0,
        })?;
    if usize::try_from(expected).ok() != Some(payload.len()) {
        return Err(FrameError::TruncatedPayload {
            expected,
            actual: payload.len(),
        });
    }

    let unpacker = Unpacker::new(payload, bpp, pixel_count);
    for (pixel, index) in unpacker.enumerate() {
        if u16::from(index) >= palette_size {
            return Err(FrameError::InvalidPaletteIndex {
                pixel: pixel as u32,
                index,
            });
        }
    }

    check_padding(payload, bpp, pixel_count)
}

/// Unpacks a validated payload into one byte per pixel.
///
/// Performs the same checks as [`validate_payload`]; `out` must hold at
/// least `pixel_count` bytes.
pub fn unpack(
    payload: &[u8],
    bpp: BitsPerPixel,
    pixel_count: u32,
    palette_size: u16,
    out: &mut [u8],
) -> Result<(), FrameError> {
    validate_payload(payload, bpp, pixel_count, palette_size)?;
    let count = usize::try_from(pixel_count).map_err(|_| FrameError::PixelCountMismatch {
        expected: pixel_count,
        actual: out.len(),
    })?;
    let available = out.len();
    let out = out.get_mut(..count).ok_or(FrameError::PixelCountMismatch {
        expected: pixel_count,
        actual: available,
    })?;
    for (dst, index) in out.iter_mut().zip(Unpacker::new(payload, bpp, pixel_count)) {
        *dst = index;
    }
    Ok(())
}

fn check_padding(payload: &[u8], bpp: BitsPerPixel, pixel_count: u32) -> Result<(), FrameError> {
    let per_byte = u32::from(bpp.pixels_per_byte());
    let used_slots = pixel_count % per_byte;
    if used_slots == 0 {
        return Ok(());
    }
    let Some(&last) = payload.last() else {
        return Ok(());
    };
    let used_bits = bpp.bits() * used_slots as u8;
    let padding_mask = 0xFFu8 >> used_bits;
    if last & padding_mask != 0 {
        return Err(FrameError::NonZeroPadding);
    }
    Ok(())
}

/// Iterator over the palette indices of a packed payload, in logical order.
///
/// The iterator does not validate indices or padding; use
/// [`validate_payload`] first, or use [`crate::Frame`], which only exists
/// for validated files. Reading past the payload yields zeros rather than
/// panicking.
#[derive(Clone, Debug)]
pub struct Unpacker<'a> {
    payload: &'a [u8],
    bpp: BitsPerPixel,
    next: u32,
    end: u32,
}

impl<'a> Unpacker<'a> {
    pub const fn new(payload: &'a [u8], bpp: BitsPerPixel, pixel_count: u32) -> Self {
        Self {
            payload,
            bpp,
            next: 0,
            end: pixel_count,
        }
    }

    /// Index of the pixel at `logical_index`, or `None` past the end.
    pub fn get(&self, logical_index: u32) -> Option<u8> {
        if logical_index >= self.end {
            return None;
        }
        let per_byte = u32::from(self.bpp.pixels_per_byte());
        let byte = self
            .payload
            .get(usize::try_from(logical_index / per_byte).ok()?)
            .copied()
            .unwrap_or(0);
        let slot = (logical_index % per_byte) as u8;
        let shift = 8 - self.bpp.bits() * (slot + 1);
        Some((byte >> shift) & self.bpp.mask())
    }
}

impl Iterator for Unpacker<'_> {
    type Item = u8;

    fn next(&mut self) -> Option<u8> {
        let index = self.get(self.next)?;
        self.next += 1;
        Some(index)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = usize::try_from(self.end - self.next).unwrap_or(usize::MAX);
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for Unpacker<'_> {}
