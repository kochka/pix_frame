//! Pixel fonts for information screens.
//!
//! A 3×5 font covers digits, upper-case letters and a few symbols; glyphs
//! are 3 pixels wide except the narrow punctuation, with one blank column
//! between glyphs. A separate bold 6×7 digit set is used where digits must
//! be readable from across the room, such as the clock.

use smart_leds::RGB8;

use crate::display::Canvas;

pub const GLYPH_HEIGHT: usize = 5;

/// Row bitmaps, most significant of the three low bits is the left column.
#[derive(Clone, Copy)]
pub struct Glyph {
    rows: [u8; GLYPH_HEIGHT],
    width: u8,
}

const fn g(width: u8, rows: [u8; GLYPH_HEIGHT]) -> Glyph {
    Glyph { rows, width }
}

/// Glyph for `c`, or `None` when the font has no such character.
pub fn glyph(c: char) -> Option<Glyph> {
    Some(match c.to_ascii_uppercase() {
        '0' => g(3, [0b111, 0b101, 0b101, 0b101, 0b111]),
        '1' => g(3, [0b010, 0b110, 0b010, 0b010, 0b111]),
        '2' => g(3, [0b111, 0b001, 0b111, 0b100, 0b111]),
        '3' => g(3, [0b111, 0b001, 0b111, 0b001, 0b111]),
        '4' => g(3, [0b101, 0b101, 0b111, 0b001, 0b001]),
        '5' => g(3, [0b111, 0b100, 0b111, 0b001, 0b111]),
        '6' => g(3, [0b111, 0b100, 0b111, 0b101, 0b111]),
        '7' => g(3, [0b111, 0b001, 0b001, 0b001, 0b001]),
        '8' => g(3, [0b111, 0b101, 0b111, 0b101, 0b111]),
        '9' => g(3, [0b111, 0b101, 0b111, 0b001, 0b111]),
        'A' => g(3, [0b010, 0b101, 0b111, 0b101, 0b101]),
        'B' => g(3, [0b110, 0b101, 0b110, 0b101, 0b110]),
        'C' => g(3, [0b111, 0b100, 0b100, 0b100, 0b111]),
        'D' => g(3, [0b110, 0b101, 0b101, 0b101, 0b110]),
        'E' => g(3, [0b111, 0b100, 0b111, 0b100, 0b111]),
        'F' => g(3, [0b111, 0b100, 0b111, 0b100, 0b100]),
        'G' => g(3, [0b111, 0b100, 0b101, 0b101, 0b111]),
        'H' => g(3, [0b101, 0b101, 0b111, 0b101, 0b101]),
        'I' => g(3, [0b111, 0b010, 0b010, 0b010, 0b111]),
        'J' => g(3, [0b001, 0b001, 0b001, 0b101, 0b111]),
        'K' => g(3, [0b101, 0b101, 0b110, 0b101, 0b101]),
        'L' => g(3, [0b100, 0b100, 0b100, 0b100, 0b111]),
        'M' => g(3, [0b101, 0b111, 0b111, 0b101, 0b101]),
        'N' => g(3, [0b110, 0b101, 0b101, 0b101, 0b101]),
        'O' => g(3, [0b111, 0b101, 0b101, 0b101, 0b111]),
        'P' => g(3, [0b111, 0b101, 0b111, 0b100, 0b100]),
        'Q' => g(3, [0b111, 0b101, 0b101, 0b111, 0b001]),
        'R' => g(3, [0b111, 0b101, 0b110, 0b101, 0b101]),
        'S' => g(3, [0b111, 0b100, 0b111, 0b001, 0b111]),
        'T' => g(3, [0b111, 0b010, 0b010, 0b010, 0b010]),
        'U' => g(3, [0b101, 0b101, 0b101, 0b101, 0b111]),
        'V' => g(3, [0b101, 0b101, 0b101, 0b101, 0b010]),
        'W' => g(3, [0b101, 0b101, 0b111, 0b111, 0b101]),
        'X' => g(3, [0b101, 0b101, 0b010, 0b101, 0b101]),
        'Y' => g(3, [0b101, 0b101, 0b010, 0b010, 0b010]),
        'Z' => g(3, [0b111, 0b001, 0b010, 0b100, 0b111]),
        ':' => g(1, [0b000, 0b001, 0b000, 0b001, 0b000]),
        '.' => g(1, [0b000, 0b000, 0b000, 0b000, 0b001]),
        '-' => g(3, [0b000, 0b000, 0b111, 0b000, 0b000]),
        '°' => g(2, [0b011, 0b011, 0b000, 0b000, 0b000]),
        '%' => g(3, [0b101, 0b001, 0b010, 0b100, 0b101]),
        ' ' => g(1, [0b000; GLYPH_HEIGHT]),
        _ => return None,
    })
}

fn glyph_or_space(c: char) -> Glyph {
    glyph(c).unwrap_or(g(1, [0; GLYPH_HEIGHT]))
}

/// Width in pixels of `text`; unknown characters count as a space.
pub fn text_width(text: &str) -> usize {
    let mut width = 0;
    for (index, c) in text.chars().enumerate() {
        if index > 0 {
            width += 1;
        }
        width += usize::from(glyph_or_space(c).width);
    }
    width
}

/// Draws `text` with its top-left corner at `(x, y)`; pixels outside the
/// canvas are dropped. Returns the width drawn.
pub fn draw_text(canvas: &mut Canvas<'_>, x: usize, y: usize, text: &str, color: RGB8) -> usize {
    let mut cursor = x;
    for (index, c) in text.chars().enumerate() {
        if index > 0 {
            cursor += 1;
        }
        let glyph = glyph_or_space(c);
        draw_rows(canvas, cursor, y, &glyph.rows, glyph.width, color);
        cursor += usize::from(glyph.width);
    }
    cursor - x
}

/// Draws row bitmaps whose low `width` bits are the columns, left to right.
fn draw_rows(canvas: &mut Canvas<'_>, x: usize, y: usize, rows: &[u8], width: u8, color: RGB8) {
    for (row, bits) in rows.iter().enumerate() {
        for column in 0..width {
            let shift = width - 1 - column;
            if (bits >> shift) & 1 == 1 {
                canvas.set(x + usize::from(column), y + row, color);
            }
        }
    }
}

pub const BIG_DIGIT_WIDTH: usize = 6;
pub const BIG_DIGIT_HEIGHT: usize = 7;
/// Blank columns between the two digits of a pair.
pub const BIG_DIGIT_GAP: usize = 2;

/// Bold digits: vertical strokes two pixels wide, horizontal strokes one.
const BIG_DIGITS: [[u8; BIG_DIGIT_HEIGHT]; 10] = [
    [
        0b011110, 0b110011, 0b110011, 0b110011, 0b110011, 0b110011, 0b011110,
    ],
    [
        0b001100, 0b011100, 0b001100, 0b001100, 0b001100, 0b001100, 0b011110,
    ],
    [
        0b011110, 0b110011, 0b000011, 0b000110, 0b001100, 0b011000, 0b111111,
    ],
    [
        0b011110, 0b110011, 0b000011, 0b001110, 0b000011, 0b110011, 0b011110,
    ],
    [
        0b000110, 0b001110, 0b011110, 0b110110, 0b111111, 0b000110, 0b000110,
    ],
    [
        0b111111, 0b110000, 0b111110, 0b000011, 0b000011, 0b110011, 0b011110,
    ],
    [
        0b001110, 0b011000, 0b110000, 0b111110, 0b110011, 0b110011, 0b011110,
    ],
    [
        0b111111, 0b000011, 0b000110, 0b001100, 0b001100, 0b001100, 0b001100,
    ],
    [
        0b011110, 0b110011, 0b110011, 0b011110, 0b110011, 0b110011, 0b011110,
    ],
    [
        0b011110, 0b110011, 0b110011, 0b011111, 0b000011, 0b000110, 0b011100,
    ],
];

/// Draws one 6×7 digit; values above 9 draw nothing.
pub fn draw_big_digit(canvas: &mut Canvas<'_>, x: usize, y: usize, digit: u8, color: RGB8) {
    if let Some(rows) = BIG_DIGITS.get(usize::from(digit)) {
        draw_rows(canvas, x, y, rows, BIG_DIGIT_WIDTH as u8, color);
    }
}

/// Width of a two-digit number drawn with [`draw_big_pair`].
pub const BIG_PAIR_WIDTH: usize = BIG_DIGIT_WIDTH * 2 + BIG_DIGIT_GAP;

/// Draws `value` (0..=99) as two 6×7 digits.
pub fn draw_big_pair(canvas: &mut Canvas<'_>, x: usize, y: usize, value: u8, color: RGB8) {
    let value = value.min(99);
    draw_big_digit(canvas, x, y, value / 10, color);
    draw_big_digit(
        canvas,
        x + BIG_DIGIT_WIDTH + BIG_DIGIT_GAP,
        y,
        value % 10,
        color,
    );
}

/// Draws `text` horizontally centered on the canvas at row `y`.
pub fn draw_text_centered(canvas: &mut Canvas<'_>, y: usize, text: &str, color: RGB8) {
    let width = text_width(text);
    let x = canvas.width().saturating_sub(width) / 2;
    draw_text(canvas, x, y, text, color);
}
