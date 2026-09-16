use core::fmt;

/// Error attached to one specific frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameError {
    /// Fewer than 8 bytes remain where a frame header is expected.
    TruncatedHeader { actual: usize },
    /// `duration_ms` is zero.
    ZeroDuration,
    /// The encoding identifier is not `RAW_INDEXED`.
    UnsupportedEncoding(u8),
    /// Reserved frame flags are set.
    UnsupportedFlags(u8),
    /// The stored payload size differs from the size derived from the header.
    PayloadSizeMismatch { expected: u32, actual: u32 },
    /// The file ends before the end of the payload.
    TruncatedPayload { expected: u32, actual: usize },
    /// A decoded index is not lower than the palette size.
    InvalidPaletteIndex { pixel: u32, index: u8 },
    /// Unused bits in the final payload byte are not zero.
    NonZeroPadding,
    /// Encoder input does not contain exactly `width × height` pixels.
    PixelCountMismatch { expected: u32, actual: usize },
}

impl FrameError {
    /// Attaches a frame number to this error.
    pub const fn at(self, frame: u32) -> Error {
        Error::Frame { frame, error: self }
    }
}

/// Any error produced while reading, validating or writing a PXA file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// Fewer than 16 bytes are available for the file header.
    TruncatedHeader { actual: usize },
    /// The first four bytes are not `PXA1`.
    InvalidMagic([u8; 4]),
    /// `width` is zero.
    ZeroWidth,
    /// `height` is zero.
    ZeroHeight,
    /// `frame_count` is zero.
    ZeroFrameCount,
    /// `palette_size` is outside `1..=256`.
    InvalidPaletteSize(u16),
    /// Reserved file flags are set.
    UnsupportedFlags(u16),
    /// The file ends inside the palette.
    TruncatedPalette { expected: usize, actual: usize },
    /// A size calculation does not fit the platform's integer types.
    Overflow,
    /// An error located in one frame (zero-based).
    Frame { frame: u32, error: FrameError },
    /// Bytes remain after the last frame.
    TrailingData { extra: usize },
    /// Encoder input palette length differs from the header.
    PaletteSizeMismatch { expected: u16, actual: usize },
    /// Encoder input frame count differs from the header.
    FrameCountMismatch { expected: u32, actual: u32 },
    /// The output buffer is too small for the encoded file.
    BufferTooSmall { required: usize, actual: usize },
}

impl fmt::Display for FrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TruncatedHeader { actual } => {
                write!(f, "truncated frame header ({actual} of 8 bytes)")
            }
            Self::ZeroDuration => write!(f, "duration_ms must not be zero"),
            Self::UnsupportedEncoding(e) => write!(f, "unsupported encoding 0x{e:02X}"),
            Self::UnsupportedFlags(flags) => {
                write!(f, "reserved frame flags set (0x{flags:02X})")
            }
            Self::PayloadSizeMismatch { expected, actual } => {
                write!(f, "payload_size is {actual}, expected {expected}")
            }
            Self::TruncatedPayload { expected, actual } => {
                write!(f, "truncated payload ({actual} of {expected} bytes)")
            }
            Self::InvalidPaletteIndex { pixel, index } => {
                write!(
                    f,
                    "pixel {pixel} uses palette index {index}, outside the palette"
                )
            }
            Self::NonZeroPadding => {
                write!(f, "padding bits in the final payload byte are not zero")
            }
            Self::PixelCountMismatch { expected, actual } => {
                write!(f, "frame has {actual} pixels, expected {expected}")
            }
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TruncatedHeader { actual } => {
                write!(f, "truncated file header ({actual} of 16 bytes)")
            }
            Self::InvalidMagic(magic) => write!(f, "invalid magic {magic:02X?}, expected \"PXA1\""),
            Self::ZeroWidth => write!(f, "width must not be zero"),
            Self::ZeroHeight => write!(f, "height must not be zero"),
            Self::ZeroFrameCount => write!(f, "frame_count must not be zero"),
            Self::InvalidPaletteSize(size) => {
                write!(f, "palette_size is {size}, must be between 1 and 256")
            }
            Self::UnsupportedFlags(flags) => write!(f, "reserved file flags set (0x{flags:04X})"),
            Self::TruncatedPalette { expected, actual } => {
                write!(f, "truncated palette ({actual} of {expected} bytes)")
            }
            Self::Overflow => write!(f, "size calculation overflow"),
            Self::Frame { frame, error } => write!(f, "frame {frame}: {error}"),
            Self::TrailingData { extra } => {
                write!(f, "{extra} trailing bytes after the last frame")
            }
            Self::PaletteSizeMismatch { expected, actual } => {
                write!(
                    f,
                    "palette has {actual} entries, header declares {expected}"
                )
            }
            Self::FrameCountMismatch { expected, actual } => {
                write!(f, "{actual} frames supplied, header declares {expected}")
            }
            Self::BufferTooSmall { required, actual } => {
                write!(f, "output buffer holds {actual} bytes, {required} required")
            }
        }
    }
}

impl core::error::Error for FrameError {}
impl core::error::Error for Error {}
