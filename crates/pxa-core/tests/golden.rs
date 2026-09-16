//! Section 19 of `doc/PXA_spec.md`: 2×2, two colors, two frames, 40 bytes.

use pxa_core::{FrameSource, Header, PxaFile, encoded_size, write};

/// Byte layout:
/// ```text
/// 50 58 41 31 02 00 02 00 02 00 00 00 02 00 00 00   header
/// 00 00 00 FF 00 00                                 palette: black, red
/// 64 00 00 00 01 00 00 00 60                        frame 0: 0,1,1,0
/// 64 00 00 00 01 00 00 00 90                        frame 1: 1,0,0,1
/// ```
pub const MINIMAL: [u8; 40] = [
    0x50, 0x58, 0x41, 0x31, 0x02, 0x00, 0x02, 0x00, 0x02, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0xFF, 0x00, 0x00, //
    0x64, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x60, //
    0x64, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x90,
];

#[test]
fn parses_minimal_example() {
    let file = PxaFile::parse(&MINIMAL).unwrap();
    let header = file.header();
    assert_eq!(header.width(), 2);
    assert_eq!(header.height(), 2);
    assert_eq!(header.frame_count(), 2);
    assert_eq!(header.palette_size(), 2);
    assert_eq!(header.bits_per_pixel(), pxa_core::BitsPerPixel::One);
    assert_eq!(header.payload_size(), Ok(1));
    assert_eq!(header.file_size(), Ok(40));

    let palette: Vec<_> = file.palette().iter().collect();
    assert_eq!(palette, vec![[0, 0, 0], [0xFF, 0, 0]]);

    let frames: Vec<_> = file.frames().collect();
    assert_eq!(frames.len(), 2);
    assert_eq!(frames[0].duration_ms(), 100);
    assert_eq!(frames[0].indices().collect::<Vec<_>>(), [0, 1, 1, 0]);
    assert_eq!(frames[1].indices().collect::<Vec<_>>(), [1, 0, 0, 1]);
    assert_eq!(
        frames[1].colors().collect::<Vec<_>>(),
        [[0xFF, 0, 0], [0, 0, 0], [0, 0, 0], [0xFF, 0, 0]]
    );

    let mut out = [0xAA; 4];
    frames[0].unpack(&mut out).unwrap();
    assert_eq!(out, [0, 1, 1, 0]);

    assert!(file.frame(2).is_none());
    assert_eq!(file.frame(1).unwrap().payload(), &[0x90]);
}

#[test]
fn encodes_minimal_example_byte_exact() {
    let header = Header::new(2, 2, 2, 2).unwrap();
    assert_eq!(encoded_size(&header), Ok(40));

    let mut out = vec![0u8; 40];
    let written = write(
        &header,
        &[[0, 0, 0], [0xFF, 0, 0]],
        [
            FrameSource {
                duration_ms: 100,
                indices: &[0, 1, 1, 0],
            },
            FrameSource {
                duration_ms: 100,
                indices: &[1, 0, 0, 1],
            },
        ],
        &mut out,
    )
    .unwrap();

    assert_eq!(written, 40);
    assert_eq!(out, MINIMAL);
}

#[test]
fn frame_offsets_follow_section_15() {
    let header = Header::parse(&MINIMAL).unwrap();
    assert_eq!(header.frame_stride(), Ok(9));
    assert_eq!(header.frame_offset(0), Ok(Some(22)));
    assert_eq!(header.frame_offset(1), Ok(Some(31)));
    assert_eq!(header.frame_offset(2), Ok(None));
}
