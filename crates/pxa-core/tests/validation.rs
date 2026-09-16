//! Section 18 of `doc/PXA_spec.md`: every rejection rule.

use pxa_core::{Error, FrameError, FrameHeader, Header, PxaFile};

const MINIMAL: [u8; 40] = [
    0x50, 0x58, 0x41, 0x31, 0x02, 0x00, 0x02, 0x00, 0x02, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0xFF, 0x00, 0x00, //
    0x64, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x60, //
    0x64, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x90,
];

fn patched(offset: usize, bytes: &[u8]) -> Vec<u8> {
    let mut file = MINIMAL.to_vec();
    file[offset..offset + bytes.len()].copy_from_slice(bytes);
    file
}

#[test]
fn decodes_little_endian_fields() {
    let header = Header::parse(&[
        b'P', b'X', b'A', b'1', 0x34, 0x12, 0x78, 0x56, 0x78, 0x56, 0x34, 0x12, 0x00, 0x01, 0x00,
        0x00,
    ])
    .unwrap();
    assert_eq!(header.width(), 0x1234);
    assert_eq!(header.height(), 0x5678);
    assert_eq!(header.frame_count(), 0x1234_5678);
    assert_eq!(header.palette_size(), 256);

    let frame = FrameHeader::parse(&[0x34, 0x12, 0x00, 0x00, 0x78, 0x56, 0x34, 0x12]).unwrap();
    assert_eq!(frame.duration_ms, 0x1234);
    assert_eq!(frame.payload_size, 0x1234_5678);
}

#[test]
fn header_roundtrips() {
    let header = Header::new(300, 7, 1000, 17).unwrap();
    assert_eq!(Header::parse(&header.to_bytes()), Ok(header));
    let frame = FrameHeader::raw_indexed(42, 99);
    assert_eq!(FrameHeader::parse(&frame.to_bytes()), Ok(frame));
}

#[test]
fn rejects_truncated_header() {
    assert_eq!(
        PxaFile::parse(&MINIMAL[..15]),
        Err(Error::TruncatedHeader { actual: 15 })
    );
    assert_eq!(
        PxaFile::parse(&[]),
        Err(Error::TruncatedHeader { actual: 0 })
    );
}

#[test]
fn rejects_invalid_magic() {
    assert_eq!(
        PxaFile::parse(&patched(3, b"2")),
        Err(Error::InvalidMagic(*b"PXA2"))
    );
    assert_eq!(
        PxaFile::parse(&patched(0, b"GIF8")),
        Err(Error::InvalidMagic(*b"GIF8"))
    );
}

#[test]
fn rejects_zero_dimensions_and_frame_count() {
    assert_eq!(PxaFile::parse(&patched(4, &[0, 0])), Err(Error::ZeroWidth));
    assert_eq!(PxaFile::parse(&patched(6, &[0, 0])), Err(Error::ZeroHeight));
    assert_eq!(
        PxaFile::parse(&patched(8, &[0, 0, 0, 0])),
        Err(Error::ZeroFrameCount)
    );
}

#[test]
fn rejects_invalid_palette_size() {
    assert_eq!(
        PxaFile::parse(&patched(12, &[0, 0])),
        Err(Error::InvalidPaletteSize(0))
    );
    assert_eq!(
        PxaFile::parse(&patched(12, &[0x01, 0x01])),
        Err(Error::InvalidPaletteSize(257))
    );
    assert_eq!(Header::new(1, 1, 1, 0), Err(Error::InvalidPaletteSize(0)));
    assert_eq!(
        Header::new(1, 1, 1, 257),
        Err(Error::InvalidPaletteSize(257))
    );
}

#[test]
fn rejects_file_flags() {
    assert_eq!(
        PxaFile::parse(&patched(14, &[0x01, 0x80])),
        Err(Error::UnsupportedFlags(0x8001))
    );
}

#[test]
fn rejects_truncated_palette() {
    assert_eq!(
        PxaFile::parse(&MINIMAL[..20]),
        Err(Error::TruncatedPalette {
            expected: 6,
            actual: 4
        })
    );
}

#[test]
fn rejects_truncated_frame_header() {
    assert_eq!(
        PxaFile::parse(&MINIMAL[..25]),
        Err(FrameError::TruncatedHeader { actual: 3 }.at(0))
    );
    assert_eq!(
        PxaFile::parse(&MINIMAL[..31]),
        Err(FrameError::TruncatedHeader { actual: 0 }.at(1))
    );
}

#[test]
fn rejects_truncated_payload() {
    assert_eq!(
        PxaFile::parse(&MINIMAL[..39]),
        Err(FrameError::TruncatedPayload {
            expected: 1,
            actual: 0
        }
        .at(1))
    );
}

#[test]
fn rejects_zero_duration() {
    assert_eq!(
        PxaFile::parse(&patched(31, &[0, 0])),
        Err(FrameError::ZeroDuration.at(1))
    );
}

#[test]
fn rejects_unknown_encoding() {
    assert_eq!(
        PxaFile::parse(&patched(24, &[0x01])),
        Err(FrameError::UnsupportedEncoding(0x01).at(0))
    );
}

#[test]
fn rejects_frame_flags() {
    assert_eq!(
        PxaFile::parse(&patched(25, &[0x04])),
        Err(FrameError::UnsupportedFlags(0x04).at(0))
    );
}

#[test]
fn rejects_payload_size_mismatch() {
    assert_eq!(
        PxaFile::parse(&patched(26, &[2, 0, 0, 0])),
        Err(FrameError::PayloadSizeMismatch {
            expected: 1,
            actual: 2
        }
        .at(0))
    );
}

#[test]
fn rejects_invalid_palette_index() {
    // 1-color palette still uses 1 bpp; index 1 is invalid.
    let mut file = patched(12, &[1, 0]);
    file.drain(19..22); // drop the red entry
    assert_eq!(
        PxaFile::parse(&file),
        Err(FrameError::InvalidPaletteIndex { pixel: 1, index: 1 }.at(0))
    );
}

#[test]
fn rejects_non_zero_padding() {
    assert_eq!(
        PxaFile::parse(&patched(39, &[0x91])),
        Err(FrameError::NonZeroPadding.at(1))
    );
}

#[test]
fn rejects_trailing_data() {
    let mut file = MINIMAL.to_vec();
    file.push(0);
    assert_eq!(PxaFile::parse(&file), Err(Error::TrailingData { extra: 1 }));
}

#[test]
fn accepts_dimensions_with_padding_bits() {
    // 3×3 at 1 bpp: 9 pixels, 2 bytes, 7 padding bits.
    let header = Header::new(3, 3, 1, 2).unwrap();
    assert_eq!(header.payload_size(), Ok(2));
    let mut file = header.to_bytes().to_vec();
    file.extend_from_slice(&[0, 0, 0, 255, 255, 255]);
    file.extend_from_slice(&FrameHeader::raw_indexed(10, 2).to_bytes());
    file.extend_from_slice(&[0b1010_1010, 0b1000_0000]);
    let parsed = PxaFile::parse(&file).unwrap();
    assert_eq!(
        parsed.frame(0).unwrap().indices().collect::<Vec<_>>(),
        [1, 0, 1, 0, 1, 0, 1, 0, 1]
    );

    file[31] = 0b1100_0000;
    assert_eq!(PxaFile::parse(&file), Err(FrameError::NonZeroPadding.at(0)));
}

#[test]
fn huge_headers_fail_fast_without_allocation() {
    // Largest possible frame and frame count, but a 16-byte file.
    let header = Header::new(u16::MAX, u16::MAX, u32::MAX, 256).unwrap();
    assert_eq!(header.pixel_count(), 65535 * 65535);
    assert_eq!(header.payload_size(), Ok(65535 * 65535));
    assert!(header.file_size().is_ok());
    assert_eq!(
        PxaFile::parse(&header.to_bytes()),
        Err(Error::TruncatedPalette {
            expected: 768,
            actual: 0
        })
    );

    // With a palette but no frames, parsing stops at frame 0.
    let mut file = header.to_bytes().to_vec();
    file.extend_from_slice(&[0; 768]);
    assert_eq!(
        PxaFile::parse(&file),
        Err(FrameError::TruncatedHeader { actual: 0 }.at(0))
    );
}

#[test]
fn errors_are_displayable() {
    let text = format!(
        "{}",
        FrameError::InvalidPaletteIndex { pixel: 7, index: 9 }.at(3)
    );
    assert_eq!(
        text,
        "frame 3: pixel 7 uses palette index 9, outside the palette"
    );
}
