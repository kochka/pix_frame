use crate::{Error, FRAME_HEADER_SIZE, FrameError, FrameHeader, HEADER_SIZE, Header, Rgb, pack};

/// One frame to encode: its duration and one palette index per pixel in
/// logical row-major order.
#[derive(Clone, Copy, Debug)]
pub struct FrameSource<'a> {
    pub duration_ms: u16,
    pub indices: &'a [u8],
}

/// Number of bytes [`write`] needs for a file with this header.
pub fn encoded_size(header: &Header) -> Result<usize, Error> {
    usize::try_from(header.file_size()?).map_err(|_| Error::Overflow)
}

/// Encodes a complete PXA v1 file into `out`.
///
/// `palette` must contain exactly `header.palette_size()` entries and
/// `frames` must yield exactly `header.frame_count()` frames of
/// `width × height` indices each. Returns the number of bytes written,
/// which equals [`encoded_size`].
pub fn write<'a, I>(
    header: &Header,
    palette: &[Rgb],
    frames: I,
    out: &mut [u8],
) -> Result<usize, Error>
where
    I: IntoIterator<Item = FrameSource<'a>>,
{
    if palette.len() != usize::from(header.palette_size()) {
        return Err(Error::PaletteSizeMismatch {
            expected: header.palette_size(),
            actual: palette.len(),
        });
    }

    let required = encoded_size(header)?;
    let available = out.len();
    let out = out.get_mut(..required).ok_or(Error::BufferTooSmall {
        required,
        actual: available,
    })?;

    let payload_size = header.payload_size()?;
    let payload_len = usize::try_from(payload_size).map_err(|_| Error::Overflow)?;
    let pixel_count = header.pixel_count();
    let bpp = header.bits_per_pixel();
    let palette_size = header.palette_size();

    let (head, rest) = out.split_at_mut(HEADER_SIZE);
    head.copy_from_slice(&header.to_bytes());
    let (palette_out, mut rest) = rest.split_at_mut(palette.len() * 3);
    for (dst, color) in palette_out.as_chunks_mut::<3>().0.iter_mut().zip(palette) {
        *dst = *color;
    }

    let mut written = 0u32;
    for (frame, source) in frames.into_iter().enumerate() {
        let frame = u32::try_from(frame).map_err(|_| Error::Overflow)?;
        if frame >= header.frame_count() {
            return Err(Error::FrameCountMismatch {
                expected: header.frame_count(),
                actual: frame.saturating_add(1),
            });
        }
        if source.indices.len() != pixel_count as usize {
            return Err(FrameError::PixelCountMismatch {
                expected: pixel_count,
                actual: source.indices.len(),
            }
            .at(frame));
        }
        if source.duration_ms == 0 {
            return Err(FrameError::ZeroDuration.at(frame));
        }

        let (frame_head, after) = rest.split_at_mut(FRAME_HEADER_SIZE);
        frame_head.copy_from_slice(
            &FrameHeader::raw_indexed(source.duration_ms, payload_size).to_bytes(),
        );
        let (payload, after) = after.split_at_mut(payload_len);
        pack(source.indices, bpp, palette_size, payload).map_err(|e| e.at(frame))?;
        rest = after;
        written += 1;
    }

    if written != header.frame_count() {
        return Err(Error::FrameCountMismatch {
            expected: header.frame_count(),
            actual: written,
        });
    }

    Ok(required)
}
