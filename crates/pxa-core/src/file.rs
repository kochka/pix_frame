use crate::{
    Error, FRAME_HEADER_SIZE, FrameError, FrameHeader, HEADER_SIZE, Header, Rgb, Unpacker, unpack,
    validate_payload,
};

/// A fully validated PXA v1 file held in memory.
///
/// [`PxaFile::parse`] performs every check listed in section 18, so the
/// accessors on this type and on [`Frame`] cannot fail on malformed data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PxaFile<'a> {
    header: Header,
    palette: &'a [u8],
    frames: &'a [u8],
    payload_size: usize,
}

impl<'a> PxaFile<'a> {
    /// Validates `bytes` as a complete PXA v1 file.
    pub fn parse(bytes: &'a [u8]) -> Result<Self, Error> {
        let header = Header::parse(bytes)?;
        let rest = &bytes[HEADER_SIZE..];

        let palette_len = header.palette_bytes();
        let palette = rest.get(..palette_len).ok_or(Error::TruncatedPalette {
            expected: palette_len,
            actual: rest.len(),
        })?;
        let mut rest = &rest[palette_len..];
        let frames = rest;

        let payload_size = header.payload_size()?;
        let payload_len = usize::try_from(payload_size).map_err(|_| Error::Overflow)?;
        let bpp = header.bits_per_pixel();
        let pixel_count = header.pixel_count();
        let palette_size = header.palette_size();

        for frame in 0..header.frame_count() {
            let frame_header = FrameHeader::parse(rest).map_err(|e| e.at(frame))?;
            frame_header
                .validate(payload_size)
                .map_err(|e| e.at(frame))?;
            rest = &rest[FRAME_HEADER_SIZE..];
            let payload = rest
                .get(..payload_len)
                .ok_or(FrameError::TruncatedPayload {
                    expected: payload_size,
                    actual: rest.len(),
                })
                .map_err(|e| e.at(frame))?;
            validate_payload(payload, bpp, pixel_count, palette_size).map_err(|e| e.at(frame))?;
            rest = &rest[payload_len..];
        }

        if !rest.is_empty() {
            return Err(Error::TrailingData { extra: rest.len() });
        }

        Ok(Self {
            header,
            palette,
            frames,
            payload_size: payload_len,
        })
    }

    pub const fn header(&self) -> &Header {
        &self.header
    }

    pub const fn palette(&self) -> Palette<'a> {
        Palette {
            bytes: self.palette,
        }
    }

    /// Iterates over all frames in file order.
    pub const fn frames(&self) -> Frames<'a> {
        Frames {
            file: *self,
            next: 0,
        }
    }

    /// Random access to frame `index`, or `None` past the last frame.
    pub fn frame(&self, index: u32) -> Option<Frame<'a>> {
        if index >= self.header.frame_count() {
            return None;
        }
        let stride = FRAME_HEADER_SIZE + self.payload_size;
        let start = usize::try_from(index).ok()?.checked_mul(stride)?;
        let bytes = self.frames.get(start..start + stride)?;
        let frame_header = FrameHeader::parse(bytes).ok()?;
        Some(Frame {
            file: *self,
            index,
            duration_ms: frame_header.duration_ms,
            payload: &bytes[FRAME_HEADER_SIZE..],
        })
    }
}

/// Global RGB888 palette of a validated file.
#[derive(Clone, Copy, Debug)]
pub struct Palette<'a> {
    bytes: &'a [u8],
}

impl<'a> Palette<'a> {
    pub const fn len(&self) -> usize {
        self.bytes.len() / 3
    }

    pub const fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Color at `index`, or `None` outside the palette.
    pub fn get(&self, index: u8) -> Option<Rgb> {
        let start = usize::from(index) * 3;
        let entry = self.bytes.get(start..start + 3)?;
        Some([entry[0], entry[1], entry[2]])
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = Rgb> + 'a {
        self.bytes.as_chunks::<3>().0.iter().copied()
    }
}

/// One validated frame.
#[derive(Clone, Copy, Debug)]
pub struct Frame<'a> {
    file: PxaFile<'a>,
    index: u32,
    duration_ms: u16,
    payload: &'a [u8],
}

impl<'a> Frame<'a> {
    /// Zero-based position in the file.
    pub const fn index(&self) -> u32 {
        self.index
    }

    pub const fn duration_ms(&self) -> u16 {
        self.duration_ms
    }

    /// Packed payload bytes.
    pub const fn payload(&self) -> &'a [u8] {
        self.payload
    }

    /// Palette indices in logical row-major order.
    pub const fn indices(&self) -> Unpacker<'a> {
        Unpacker::new(
            self.payload,
            self.file.header.bits_per_pixel(),
            self.file.header.pixel_count(),
        )
    }

    /// RGB colors in logical row-major order.
    pub fn colors(&self) -> impl Iterator<Item = Rgb> + 'a {
        let palette = self.file.palette();
        self.indices()
            .map(move |index| palette.get(index).unwrap_or([0, 0, 0]))
    }

    /// Writes one palette index per pixel into `out`.
    ///
    /// `out` must hold at least `width × height` bytes.
    pub fn unpack(&self, out: &mut [u8]) -> Result<(), Error> {
        let header = &self.file.header;
        unpack(
            self.payload,
            header.bits_per_pixel(),
            header.pixel_count(),
            header.palette_size(),
            out,
        )
        .map_err(|e| e.at(self.index))
    }
}

/// Iterator over the frames of a validated file.
#[derive(Clone, Copy, Debug)]
pub struct Frames<'a> {
    file: PxaFile<'a>,
    next: u32,
}

impl<'a> Iterator for Frames<'a> {
    type Item = Frame<'a>;

    fn next(&mut self) -> Option<Frame<'a>> {
        let frame = self.file.frame(self.next)?;
        self.next += 1;
        Some(frame)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.file.header.frame_count() - self.next;
        let remaining = usize::try_from(remaining).unwrap_or(usize::MAX);
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for Frames<'_> {}
