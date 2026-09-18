//! Streams one animation frame by frame from a [`ByteSource`].
//!
//! Memory use is bounded by the display, not by the animation: one palette
//! (at most 768 bytes) and one packed payload (at most one byte per pixel).
//! Every frame is validated with `pxa-core` before it is rendered, so a
//! corrupted file yields an error rather than garbage or a panic.

use pxa_core::{
    Error, FRAME_HEADER_SIZE, FrameError, FrameHeader, HEADER_SIZE, Header, MAX_PALETTE_SIZE, Rgb,
    Unpacker, validate_payload,
};
use smart_leds::RGB8;

use crate::catalog::Entry;
use crate::display::MatrixLayout;
use crate::storage::{ByteSource, StorageError};

#[derive(Clone, Copy, Debug)]
pub enum PlayError {
    Storage(StorageError),
    Format(Error),
    Frame {
        index: u32,
        error: FrameError,
    },
    /// The animation does not have the dimensions of the display.
    SizeMismatch {
        width: u16,
        height: u16,
    },
    /// The frame payload is larger than the player's buffer.
    PayloadTooLarge {
        payload_size: u32,
    },
    /// Internal inconsistency: the frame index ran past `frame_count`.
    NoSuchFrame(u32),
}

impl From<StorageError> for PlayError {
    fn from(error: StorageError) -> Self {
        Self::Storage(error)
    }
}

impl defmt::Format for PlayError {
    fn format(&self, f: defmt::Formatter<'_>) {
        match self {
            Self::Storage(error) => defmt::write!(f, "read error: {}", error),
            Self::Format(error) => defmt::write!(f, "{}", defmt::Display2Format(error)),
            Self::Frame { index, error } => {
                defmt::write!(f, "frame {}: {}", index, defmt::Display2Format(error));
            }
            Self::SizeMismatch { width, height } => {
                defmt::write!(f, "animation is {}x{}, not the display size", width, height);
            }
            Self::PayloadTooLarge { payload_size } => {
                defmt::write!(
                    f,
                    "frame payload of {} bytes exceeds the buffer",
                    payload_size
                );
            }
            Self::NoSuchFrame(index) => defmt::write!(f, "no frame {}", index),
        }
    }
}

/// What [`Player::next_frame`] just rendered.
#[derive(Clone, Copy, Debug)]
pub struct FrameInfo {
    pub index: u32,
    pub duration_ms: u16,
    /// This was the last frame; the next call wraps to the first one.
    pub is_last: bool,
}

/// Playback state of one animation.
///
/// `N` is the number of pixels of the display and bounds the payload buffer
/// (one byte per pixel at 8 bpp).
pub struct Player<const N: usize> {
    header: Header,
    /// Offset of the file in the source.
    base: u32,
    payload_size: u32,
    palette: [Rgb; MAX_PALETTE_SIZE as usize],
    payload: [u8; N],
    next: u32,
}

impl<const N: usize> Player<N> {
    /// Checks the animation against the display and loads its palette.
    pub fn open<S: ByteSource>(
        source: &mut S,
        entry: &Entry,
        layout: &MatrixLayout,
    ) -> Result<Self, PlayError> {
        let header = entry.header;
        if usize::from(header.width()) != layout.width
            || usize::from(header.height()) != layout.height
        {
            return Err(PlayError::SizeMismatch {
                width: header.width(),
                height: header.height(),
            });
        }
        let payload_size = header.payload_size().map_err(PlayError::Format)?;
        if !usize::try_from(payload_size).is_ok_and(|size| size <= N) {
            return Err(PlayError::PayloadTooLarge { payload_size });
        }

        let mut player = Self {
            header,
            base: entry.offset,
            payload_size,
            palette: [[0; 3]; MAX_PALETTE_SIZE as usize],
            payload: [0; N],
            next: 0,
        };
        let palette_offset = entry
            .offset
            .checked_add(HEADER_SIZE as u32)
            .ok_or(PlayError::Format(Error::Overflow))?;
        let palette_bytes = header.palette_bytes();
        source.read(
            palette_offset,
            &mut player.palette.as_flattened_mut()[..palette_bytes],
        )?;
        Ok(player)
    }

    pub fn header(&self) -> &Header {
        &self.header
    }

    /// Reads, validates and renders the next frame into `pixels`, mapping
    /// logical coordinates through `layout`. Wraps around after the last frame.
    pub fn next_frame<S: ByteSource>(
        &mut self,
        source: &mut S,
        layout: &MatrixLayout,
        pixels: &mut [RGB8; N],
    ) -> Result<FrameInfo, PlayError> {
        let index = self.next;
        let frame_error = |error| PlayError::Frame { index, error };

        let relative = self
            .header
            .frame_offset(index)
            .map_err(PlayError::Format)?
            .ok_or(PlayError::NoSuchFrame(index))?;
        let offset = u32::try_from(u64::from(self.base) + relative)
            .map_err(|_| PlayError::Format(Error::Overflow))?;

        let mut raw = [0u8; FRAME_HEADER_SIZE];
        source.read(offset, &mut raw)?;
        let frame_header = FrameHeader::parse(&raw).map_err(frame_error)?;
        frame_header
            .validate(self.payload_size)
            .map_err(frame_error)?;

        // Checked against `N` in `open`.
        let payload = &mut self.payload[..self.payload_size as usize];
        source.read(offset + FRAME_HEADER_SIZE as u32, payload)?;

        let bpp = self.header.bits_per_pixel();
        let pixel_count = self.header.pixel_count();
        validate_payload(payload, bpp, pixel_count, self.header.palette_size())
            .map_err(frame_error)?;

        for (logical, color) in Unpacker::new(payload, bpp, pixel_count).enumerate() {
            let (x, y) = (logical % layout.width, logical / layout.width);
            let Some(physical) = layout.physical_index(x, y) else {
                continue;
            };
            if let Some(pixel) = pixels.get_mut(physical) {
                let [r, g, b] = self.palette[usize::from(color)];
                *pixel = RGB8::new(r, g, b);
            }
        }

        let frame_count = self.header.frame_count();
        let is_last = index + 1 >= frame_count;
        self.next = if is_last { 0 } else { index + 1 };
        Ok(FrameInfo {
            index,
            duration_ms: frame_header.duration_ms,
            is_last,
        })
    }
}
