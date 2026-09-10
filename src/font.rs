//! Bitmap glyph font for the big clock.
//!
//! Each digit is a 5x7 on/off pixel bitmap (tty-clock lineage: a low-res
//! bitmap scaled up into blocks). `1` in a row means "pixel lit". The colon
//! is a narrow 2-wide glyph so it reads as stacked dots between fields.

/// Height of every glyph, in pixels.
pub const GLYPH_H: usize = 7;

/// Pixel rows for a clock character. Digits are 5 wide; the colon is a slim
/// 1-wide pair of dots so it never crowds the digits. Unknown chars are blank.
pub fn glyph(c: char) -> &'static [&'static str] {
    match c {
        '0' => &[
            "01110", "10001", "10011", "10101", "11001", "10001", "01110",
        ],
        '1' => &[
            "00100", "01100", "00100", "00100", "00100", "00100", "01110",
        ],
        '2' => &[
            "01110", "10001", "00001", "00110", "01000", "10000", "11111",
        ],
        '3' => &[
            "01110", "10001", "00001", "00110", "00001", "10001", "01110",
        ],
        '4' => &[
            "10001", "10001", "10001", "11111", "00001", "00001", "00001",
        ],
        '5' => &[
            "11111", "10000", "11110", "00001", "00001", "10001", "01110",
        ],
        '6' => &[
            "01110", "10001", "10000", "11110", "10001", "10001", "01110",
        ],
        '7' => &[
            "11111", "00001", "00010", "00100", "01000", "01000", "01000",
        ],
        '8' => &[
            "01110", "10001", "10001", "01110", "10001", "10001", "01110",
        ],
        '9' => &[
            "01110", "10001", "10001", "01111", "00001", "10001", "01110",
        ],
        ':' => &["0", "1", "0", "0", "0", "1", "0"],
        ' ' => &[
            "00000", "00000", "00000", "00000", "00000", "00000", "00000",
        ],
        _ => &[
            "00000", "00000", "00000", "00000", "00000", "00000", "00000",
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_clock_char_has_full_height() {
        for c in "0123456789: ".chars() {
            assert_eq!(glyph(c).len(), GLYPH_H, "glyph {c:?} wrong height");
        }
    }

    #[test]
    fn rows_are_rectangular() {
        for c in "0123456789".chars() {
            let g = glyph(c);
            let w = g[0].len();
            assert_eq!(w, 5, "digit {c:?} not 5 wide");
            assert!(g.iter().all(|r| r.len() == w), "digit {c:?} ragged");
            assert!(
                g.iter()
                    .flat_map(|r| r.chars())
                    .all(|p| p == '0' || p == '1'),
                "digit {c:?} has non-bit pixel"
            );
        }
    }
}
