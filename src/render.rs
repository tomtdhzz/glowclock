//! Turn a `HH:MM:SS` string into big, gradient-shaded lines.
//!
//! Rendering strategy (the "fancy" part):
//!   * Each glyph pixel is scaled to `PX_W` columns x `PX_H` rows of solid
//!     block characters, giving the chunky pixel-art body.
//!   * Colour is a vertical gradient: the top output row uses the theme's
//!     `top` colour and the bottom row its `bottom` colour, interpolated in
//!     RGB per output row. This is what reads as the glow in truecolor terms.

use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

use crate::font::{glyph, GLYPH_H};

/// Solid cell for a lit pixel.
const CELL: char = '█';

/// How much to magnify one glyph pixel into terminal cells.
#[derive(Clone, Copy)]
pub struct Scale {
    /// Columns of blocks per lit pixel.
    pub px_w: usize,
    /// Rows of blocks per lit pixel.
    pub px_h: usize,
    /// Blank pixel-columns between adjacent glyphs.
    pub gap: usize,
}

/// Default scale used by non-interactive modes (snapshot/gallery/plain).
pub const DEFAULT_SCALE: Scale = Scale {
    px_w: 2,
    px_h: 2,
    gap: 1,
};

/// Candidate scales, largest first. `best_scale` walks these to find the
/// biggest one that fits the available area. The ceiling is deliberately
/// modest so the clock stays readable without dominating the window.
const CANDIDATES: &[Scale] = &[
    Scale {
        px_w: 2,
        px_h: 2,
        gap: 1,
    },
    Scale {
        px_w: 2,
        px_h: 1,
        gap: 1,
    },
    Scale {
        px_w: 1,
        px_h: 2,
        gap: 1,
    },
    Scale {
        px_w: 1,
        px_h: 1,
        gap: 1,
    },
];

/// Total rendered height in terminal rows for `scale`.
pub fn render_height(scale: Scale) -> usize {
    GLYPH_H * scale.px_h
}

/// Largest candidate scale whose rendered `text` fits within `max_w` columns
/// and `max_h` rows. Falls back to the smallest candidate if none fit.
pub fn best_scale(text: &str, max_w: usize, max_h: usize) -> Scale {
    CANDIDATES
        .iter()
        .copied()
        .find(|s| clock_width(text, *s) <= max_w && render_height(*s) <= max_h)
        .unwrap_or_else(|| *CANDIDATES.last().unwrap())
}

/// A named colour scheme: a top→bottom RGB gradient over the digits.
#[derive(Clone, Copy)]
pub struct Theme {
    pub name: &'static str,
    pub top: (u8, u8, u8),
    pub bottom: (u8, u8, u8),
    /// Dark canvas colour behind the digits.
    pub bg: (u8, u8, u8),
}

/// Built-in themes; cycle through them to compare beautification options.
pub const THEMES: &[Theme] = &[
    Theme {
        name: "aurora",
        top: (128, 245, 224),
        bottom: (46, 96, 224),
        bg: (12, 14, 20),
    },
    Theme {
        name: "sunset",
        top: (255, 214, 120),
        bottom: (232, 62, 122),
        bg: (20, 12, 18),
    },
    Theme {
        name: "matrix",
        top: (190, 255, 190),
        bottom: (18, 122, 44),
        bg: (8, 14, 8),
    },
    Theme {
        name: "ice",
        top: (238, 242, 248),
        bottom: (108, 132, 176),
        bg: (12, 14, 20),
    },
];

/// Linearly interpolate two RGB colours; `t` in `0.0..=1.0`.
fn lerp(a: (u8, u8, u8), b: (u8, u8, u8), t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    let mix = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color::Rgb(mix(a.0, b.0), mix(a.1, b.1), mix(a.2, b.2))
}

/// Render `text` (digits, `:`, spaces) as big gradient lines at `scale`.
pub fn clock_lines(text: &str, theme: Theme, scale: Scale) -> Vec<Line<'static>> {
    let chars: Vec<char> = text.chars().collect();
    let height = render_height(scale);
    let mut lines = Vec::with_capacity(height);

    for out_row in 0..height {
        let pix_row = out_row / scale.px_h;
        // Gradient position over the full rendered height for smoothness.
        let t = if height > 1 {
            out_row as f32 / (height - 1) as f32
        } else {
            0.0
        };
        let lit = Style::default().fg(lerp(theme.top, theme.bottom, t));

        let mut spans: Vec<Span<'static>> = Vec::new();
        let mut pending_gap = String::new();

        for (i, &c) in chars.iter().enumerate() {
            let g = glyph(c);
            let bits = g[pix_row];
            for bit in bits.chars() {
                let block: String =
                    std::iter::repeat_n(if bit == '1' { CELL } else { ' ' }, scale.px_w).collect();
                if bit == '1' {
                    if !pending_gap.is_empty() {
                        spans.push(Span::raw(std::mem::take(&mut pending_gap)));
                    }
                    spans.push(Span::styled(block, lit));
                } else {
                    pending_gap.push_str(&block);
                }
            }
            if i + 1 < chars.len() {
                pending_gap.push_str(&" ".repeat(scale.px_w * scale.gap));
            }
        }
        if !pending_gap.is_empty() {
            spans.push(Span::raw(pending_gap));
        }
        lines.push(Line::from(spans));
    }
    lines
}

/// Rendered pixel width of `text` in terminal columns at `scale`.
pub fn clock_width(text: &str, scale: Scale) -> usize {
    let mut w = 0usize;
    let chars: Vec<char> = text.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        w += glyph(c)[0].len() * scale.px_w;
        if i + 1 < chars.len() {
            w += scale.px_w * scale.gap;
        }
    }
    w
}

/// Emit one frame as raw truecolor ANSI to `out` (no alt-screen / raw mode),
/// so it can be captured or piped. Includes a dark background band.
pub fn write_ansi(
    out: &mut impl std::io::Write,
    text: &str,
    theme: Theme,
    scale: Scale,
) -> std::io::Result<()> {
    let (br, bg, bb) = theme.bg;
    let width = clock_width(text, scale);
    let pad = 2usize;
    for line in clock_lines(text, theme, scale) {
        write!(out, "\x1b[48;2;{br};{bg};{bb}m")?;
        write!(out, "{}", " ".repeat(pad))?;
        let mut used = 0usize;
        for span in line.spans {
            used += span.content.chars().count();
            match span.style.fg {
                Some(Color::Rgb(r, g, b)) => {
                    write!(out, "\x1b[38;2;{r};{g};{b}m{}", span.content)?;
                }
                _ => write!(out, "\x1b[39m{}", span.content)?,
            }
        }
        // Pad the background band to a stable width.
        if used < width {
            write!(out, "\x1b[39m{}", " ".repeat(width - used))?;
        }
        write!(out, "{}", " ".repeat(pad))?;
        writeln!(out, "\x1b[0m")?;
    }
    out.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_expected_height() {
        let lines = clock_lines("12:34:56", THEMES[0], DEFAULT_SCALE);
        assert_eq!(lines.len(), render_height(DEFAULT_SCALE));
    }

    #[test]
    fn width_matches_digit_layout() {
        // 8 chars: 6 digits (5px) + 2 colons (1px) + 7 gaps, scaled.
        let s = DEFAULT_SCALE;
        let expected = ((6 * 5 + 2) + 7 * s.gap) * s.px_w;
        assert_eq!(clock_width("12:34:56", s), expected);
    }

    #[test]
    fn top_row_uses_theme_top_colour() {
        let lines = clock_lines("8", THEMES[0], DEFAULT_SCALE);
        let top_lit = lines[0]
            .spans
            .iter()
            .find_map(|s| s.style.fg)
            .expect("a lit span on the top row");
        assert_eq!(top_lit, Color::Rgb(128, 245, 224));
    }

    #[test]
    fn best_scale_shrinks_to_fit_width() {
        // Full clock at the largest scale is wide; a narrow area must pick
        // a smaller candidate that actually fits.
        let narrow = 40;
        let s = best_scale("12:34:56", narrow, 100);
        assert!(clock_width("12:34:56", s) <= narrow);
    }

    #[test]
    fn best_scale_falls_back_to_smallest() {
        // Impossibly tight area still yields the smallest candidate,
        // which always keeps a gap so digits never mash together.
        let s = best_scale("12:34:56", 1, 1);
        assert_eq!((s.px_w, s.px_h, s.gap), (1, 1, 1));
    }

    #[test]
    fn smallest_scale_never_uses_zero_gap() {
        assert!(CANDIDATES.iter().all(|s| s.gap >= 1));
    }
}
