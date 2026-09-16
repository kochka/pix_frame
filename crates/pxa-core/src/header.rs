use crate::{
    BitsPerPixel, ENCODING_RAW_INDEXED, Error, FRAME_HEADER_SIZE, FrameError, HEADER_SIZE, MAGIC,
    MAX_PALETTE_SIZE,
};

/// Validated PXA v1 file header.
///
/// A `Header` can only be obtained through [`Header::new`] or
/// [`Header::parse`], both of which enforce the rules of section 7 and 18.
/// Derived quantities such as the bit depth therefore never fail.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header {
    width: u16,
    height: u16,
    frame_count: u32,
    palette_size: u16,
}

impl Header {
    /// Builds a header and checks that every field is valid for PXA v1.
    pub const fn new(
        width: u16,
        height: u16,
        frame_count: u32,
        palette_size: u16,
    ) -> Result<Self, Error> {
        if width == 0 {
            return Err(Error::ZeroWidth);
        }
        if height == 0 {
            return Err(Error::ZeroHeight);
        }
        if frame_count == 0 {
            return Err(Error::ZeroFrameCount);
        }
        if palette_size == 0 || palette_size > MAX_PALETTE_SIZE {
            return Err(Error::InvalidPaletteSize(palette_size));
        }
        Ok(Self {
            width,
            height,
            frame_count,
            palette_size,
        })
    }

    /// Parses and validates the 16-byte file header at the start of `bytes`.
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() < HEADER_SIZE {
            return Err(Error::TruncatedHeader {
                actual: bytes.len(),
            });
        }

        let magic = [bytes[0], bytes[1], bytes[2], bytes[3]];
        if magic != MAGIC {
            return Err(Error::InvalidMagic(magic));
        }

        let width = u16::from_le_bytes([bytes[4], bytes[5]]);
        let height = u16::from_le_bytes([bytes[6], bytes[7]]);
        let frame_count = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
        let palette_size = u16::from_le_bytes([bytes[12], bytes[13]]);
        let flags = u16::from_le_bytes([bytes[14], bytes[15]]);

        if flags != 0 {
            return Err(Error::UnsupportedFlags(flags));
        }

        Self::new(width, height, frame_count, palette_size)
    }

    /// Serializes the header, with zero flags.
    pub fn to_bytes(&self) -> [u8; HEADER_SIZE] {
        let mut out = [0u8; HEADER_SIZE];
        out[0..4].copy_from_slice(&MAGIC);
        out[4..6].copy_from_slice(&self.width.to_le_bytes());
        out[6..8].copy_from_slice(&self.height.to_le_bytes());
        out[8..12].copy_from_slice(&self.frame_count.to_le_bytes());
        out[12..14].copy_from_slice(&self.palette_size.to_le_bytes());
        out
    }

    pub const fn width(&self) -> u16 {
        self.width
    }

    pub const fn height(&self) -> u16 {
        self.height
    }

    pub const fn frame_count(&self) -> u32 {
        self.frame_count
    }

    pub const fn palette_size(&self) -> u16 {
        self.palette_size
    }

    /// Bit depth derived from the palette size.
    pub const fn bits_per_pixel(&self) -> BitsPerPixel {
        match BitsPerPixel::from_palette_size(self.palette_size) {
            Some(bpp) => bpp,
            // Unreachable: the constructor rejects invalid palette sizes.
            None => BitsPerPixel::Eight,
        }
    }

    /// `width × height`. Always fits in `u32` for 16-bit dimensions.
    pub const fn pixel_count(&self) -> u32 {
        self.width as u32 * self.height as u32
    }

    /// Number of bytes occupied by the palette.
    pub const fn palette_bytes(&self) -> usize {
        self.palette_size as usize * 3
    }

    /// Exact `RAW_INDEXED` payload size of every frame.
    pub const fn payload_size(&self) -> Result<u32, Error> {
        match self.bits_per_pixel().packed_len(self.pixel_count()) {
            Some(size) => Ok(size),
            None => Err(Error::Overflow),
        }
    }

    /// Frame header plus payload, in bytes.
    pub const fn frame_stride(&self) -> Result<u64, Error> {
        match self.payload_size() {
            Ok(size) => Ok(FRAME_HEADER_SIZE as u64 + size as u64),
            Err(e) => Err(e),
        }
    }

    /// Byte offset of the header of frame `index` from the start of the file.
    ///
    /// Returns `None` when `index` is not lower than `frame_count`.
    pub fn frame_offset(&self, index: u32) -> Result<Option<u64>, Error> {
        if index >= self.frame_count {
            return Ok(None);
        }
        let stride = self.frame_stride()?;
        let frames = u64::from(index)
            .checked_mul(stride)
            .ok_or(Error::Overflow)?;
        (HEADER_SIZE as u64 + self.palette_bytes() as u64)
            .checked_add(frames)
            .map(Some)
            .ok_or(Error::Overflow)
    }

    /// Total size of a conforming file with this header.
    pub fn file_size(&self) -> Result<u64, Error> {
        let stride = self.frame_stride()?;
        let frames = u64::from(self.frame_count)
            .checked_mul(stride)
            .ok_or(Error::Overflow)?;
        (HEADER_SIZE as u64 + self.palette_bytes() as u64)
            .checked_add(frames)
            .ok_or(Error::Overflow)
    }
}

/// PXA v1 frame header, as stored in the file.
///
/// Unlike [`Header`], parsing does not validate the fields: a diagnostic
/// tool may want to display an invalid header. Call
/// [`FrameHeader::validate`] before trusting it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameHeader {
    pub duration_ms: u16,
    pub encoding: u8,
    pub flags: u8,
    pub payload_size: u32,
}

impl FrameHeader {
    /// Builds a `RAW_INDEXED` frame header with zero flags.
    pub const fn raw_indexed(duration_ms: u16, payload_size: u32) -> Self {
        Self {
            duration_ms,
            encoding: ENCODING_RAW_INDEXED,
            flags: 0,
            payload_size,
        }
    }

    /// Decodes the 8-byte frame header at the start of `bytes` without validating it.
    pub fn parse(bytes: &[u8]) -> Result<Self, FrameError> {
        if bytes.len() < FRAME_HEADER_SIZE {
            return Err(FrameError::TruncatedHeader {
                actual: bytes.len(),
            });
        }
        Ok(Self {
            duration_ms: u16::from_le_bytes([bytes[0], bytes[1]]),
            encoding: bytes[2],
            flags: bytes[3],
            payload_size: u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
        })
    }

    /// Checks the header against PXA v1 rules and the payload size derived
    /// from the file header.
    pub const fn validate(&self, expected_payload_size: u32) -> Result<(), FrameError> {
        if self.duration_ms == 0 {
            return Err(FrameError::ZeroDuration);
        }
        if self.encoding != ENCODING_RAW_INDEXED {
            return Err(FrameError::UnsupportedEncoding(self.encoding));
        }
        if self.flags != 0 {
            return Err(FrameError::UnsupportedFlags(self.flags));
        }
        if self.payload_size != expected_payload_size {
            return Err(FrameError::PayloadSizeMismatch {
                expected: expected_payload_size,
                actual: self.payload_size,
            });
        }
        Ok(())
    }

    /// Serializes the frame header.
    pub fn to_bytes(&self) -> [u8; FRAME_HEADER_SIZE] {
        let mut out = [0u8; FRAME_HEADER_SIZE];
        out[0..2].copy_from_slice(&self.duration_ms.to_le_bytes());
        out[2] = self.encoding;
        out[3] = self.flags;
        out[4..8].copy_from_slice(&self.payload_size.to_le_bytes());
        out
    }
}
