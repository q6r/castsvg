//! Render a `term::Model` into a single self-contained animated SVG.
//!
//! Each frame is a `<g>` group; CSS `@keyframes` toggle each group's opacity so
//! exactly one is visible at a time. The result loops forever with no scripts,
//! no external assets — it drops straight into a README.

use crate::term::{Cell, Color, Model};
use std::fmt::Write;

pub struct Options {
    pub font_size: f64,
    pub theme: Theme,
    pub looping: bool,
}

#[derive(Clone, Copy)]
pub struct Theme {
    pub bg: &'static str,
    pub fg: &'static str,
}

impl Theme {
    pub fn from_name(name: &str) -> Option<Theme> {
        match name {
            "dark" => Some(Theme {
                bg: "#1e1e1e",
                fg: "#d4d4d4",
            }),
            "light" => Some(Theme {
                bg: "#ffffff",
                fg: "#333333",
            }),
            _ => None,
        }
    }
}

// The 16 ANSI colours (VS Code's terminal palette — readable on either theme).
const ANSI16: [&str; 16] = [
    "#000000", "#cd3131", "#0dbc79", "#e5e510", "#2472c8", "#bc3fbc", "#11a8cd", "#e5e5e5",
    "#666666", "#f14c4c", "#23d18b", "#f5f543", "#3b8eea", "#d670d6", "#29b8db", "#ffffff",
];

fn indexed_hex(n: u8) -> String {
    match n {
        0..=15 => ANSI16[n as usize].to_string(),
        16..=231 => {
            let n = n as u16 - 16;
            let steps = |v: u16| -> u8 {
                if v == 0 {
                    0
                } else {
                    (55 + 40 * v) as u8
                }
            };
            let r = steps(n / 36);
            let g = steps((n / 6) % 6);
            let b = steps(n % 6);
            format!("#{:02x}{:02x}{:02x}", r, g, b)
        }
        _ => {
            let v = 8 + 10 * (n as u16 - 232);
            format!("#{0:02x}{0:02x}{0:02x}", v as u8)
        }
    }
}

fn resolve(color: Color, is_fg: bool, theme: &Theme) -> String {
    match color {
        Color::Default => {
            if is_fg {
                theme.fg.to_string()
            } else {
                theme.bg.to_string()
            }
        }
        Color::Indexed(n) => indexed_hex(n),
        Color::Rgb(r, g, b) => format!("#{:02x}{:02x}{:02x}", r, g, b),
    }
}

/// The displayed (post-inverse) foreground and background hex for a cell.
fn cell_colors(cell: &Cell, theme: &Theme) -> (String, String) {
    let fg = resolve(cell.fg, true, theme);
    let bg = resolve(cell.bg, false, theme);
    if cell.inverse {
        (bg, fg)
    } else {
        (fg, bg)
    }
}

fn escape(s: &str, out: &mut String) {
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(c),
        }
    }
}

/// Build the `@keyframes` stops for a frame visible over `[start, end]` percent.
fn keyframes(start: f64, end: f64, hold_to_end: bool) -> String {
    let e = 0.02_f64;
    let mut stops: Vec<(f64, u8)> = Vec::new();
    if start <= 0.0 {
        stops.push((0.0, 1));
    } else {
        stops.push((0.0, 0));
        let a = (start - e).max(0.0);
        if a > 0.0 {
            stops.push((a, 0));
        }
        stops.push((start, 1));
    }
    stops.push((end, 1));
    if hold_to_end || end >= 100.0 {
        stops.push((100.0, 1));
    } else {
        stops.push(((end + e).min(100.0), 0));
        stops.push((100.0, 0));
    }

    let mut out = String::new();
    let mut last_key: Option<String> = None;
    for (p, v) in stops {
        let key = format!("{:.3}", p.clamp(0.0, 100.0));
        if last_key.as_deref() == Some(key.as_str()) {
            continue;
        }
        let _ = write!(out, "{}%{{opacity:{}}}", key, v);
        last_key = Some(key);
    }
    out
}

pub fn render(model: &Model, opts: &Options) -> String {
    let fs = opts.font_size;
    let cw = fs * 0.6; // monospace advance width
    let lh = fs * 1.2; // line height
    let pad = fs * 0.9;
    let width = model.cols as f64 * cw + 2.0 * pad;
    let height = model.rows as f64 * lh + 2.0 * pad;
    let theme = &opts.theme;

    let total_ms: f64 = model
        .frames
        .iter()
        .map(|f| f.duration_ms)
        .sum::<f64>()
        .max(1.0);
    let total_s = total_ms / 1000.0;

    let mut svg = String::new();
    let _ = write!(
        svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w:.1} {h:.1}" width="{w:.0}" height="{h:.0}" font-family="'DejaVu Sans Mono','Cascadia Code',Menlo,Consolas,'Courier New',monospace" font-size="{fs}">"#,
        w = width,
        h = height,
        fs = fs
    );

    // --- styles / animation ---
    svg.push_str("<style>");
    let _ = write!(svg, ".f{{opacity:0}}text{{white-space:pre}}");
    let iter = if opts.looping { "infinite" } else { "1 both" };
    let mut cum = 0.0_f64;
    for (i, frame) in model.frames.iter().enumerate() {
        let start = cum / total_ms * 100.0;
        cum += frame.duration_ms;
        let end = cum / total_ms * 100.0;
        let last = i == model.frames.len() - 1;
        let hold = last && !opts.looping;
        let _ = write!(
            svg,
            ".f{i}{{animation:k{i} {ts:.3}s {iter} linear}}@keyframes k{i}{{{body}}}",
            i = i,
            ts = total_s,
            iter = iter,
            body = keyframes(start, end, hold)
        );
    }
    svg.push_str("</style>");

    // --- background ---
    let _ = write!(
        svg,
        r#"<rect width="100%" height="100%" fill="{}" rx="{:.1}"/>"#,
        theme.bg,
        fs * 0.4
    );

    // --- frames ---
    for (i, frame) in model.frames.iter().enumerate() {
        let _ = write!(svg, r#"<g class="f f{}">"#, i);
        render_frame(&mut svg, frame.cells.as_slice(), model, opts, cw, lh, pad);
        svg.push_str("</g>");
    }

    svg.push_str("</svg>");
    svg
}

fn render_frame(
    svg: &mut String,
    cells: &[Cell],
    model: &Model,
    opts: &Options,
    cw: f64,
    lh: f64,
    pad: f64,
) {
    let theme = &opts.theme;
    let cols = model.cols;

    for row in 0..model.rows {
        let base = row * cols;
        let y_top = pad + row as f64 * lh;

        // Precompute displayed colours for the row.
        let mut fg = Vec::with_capacity(cols);
        let mut bg = Vec::with_capacity(cols);
        for col in 0..cols {
            let (f, b) = cell_colors(&cells[base + col], theme);
            fg.push(f);
            bg.push(b);
        }

        // Background rects: runs of identical non-default background.
        let mut col = 0;
        while col < cols {
            let this = &bg[col];
            if this == theme.bg {
                col += 1;
                continue;
            }
            let start = col;
            while col < cols && &bg[col] == this {
                col += 1;
            }
            let _ = write!(
                svg,
                r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" fill="{}"/>"#,
                pad + start as f64 * cw,
                y_top,
                (col - start) as f64 * cw,
                lh,
                this
            );
        }

        // Text: runs of identical foreground + weight containing a glyph.
        let baseline = y_top + opts.font_size * 0.78;
        let mut col = 0;
        while col < cols {
            let start = col;
            let cur_fg = fg[col].clone();
            let cur_bold = cells[base + col].bold;
            let mut text = String::new();
            let mut has_glyph = false;
            while col < cols && fg[col] == cur_fg && cells[base + col].bold == cur_bold {
                let ch = cells[base + col].ch;
                if ch != ' ' {
                    has_glyph = true;
                }
                text.push(ch);
                col += 1;
            }
            if !has_glyph {
                continue;
            }
            let weight = if cur_bold {
                r#" font-weight="bold""#
            } else {
                ""
            };
            let _ = write!(
                svg,
                r#"<text x="{:.2}" y="{:.2}" fill="{}" textLength="{:.2}" lengthAdjust="spacingAndGlyphs"{}>"#,
                pad + start as f64 * cw,
                baseline,
                cur_fg,
                (col - start) as f64 * cw,
                weight
            );
            escape(&text, svg);
            svg.push_str("</text>");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_anchors() {
        // Known fixed points of the xterm 256-colour cube.
        assert_eq!(indexed_hex(0), "#000000");
        assert_eq!(indexed_hex(15), "#ffffff");
        assert_eq!(indexed_hex(16), "#000000"); // cube origin
        assert_eq!(indexed_hex(231), "#ffffff"); // cube corner
        assert_eq!(indexed_hex(196), "#ff0000"); // pure red in the cube
        assert_eq!(indexed_hex(232), "#080808"); // first grey ramp step
        assert_eq!(indexed_hex(255), "#eeeeee"); // last grey ramp step
    }

    #[test]
    fn inverse_swaps_fg_and_bg() {
        let theme = Theme::from_name("dark").unwrap();
        let cell = Cell {
            ch: 'x',
            fg: Color::Indexed(1),
            bg: Color::Default,
            bold: false,
            inverse: true,
        };
        let (fg, bg) = cell_colors(&cell, &theme);
        assert_eq!(fg, theme.bg); // default bg becomes the text colour
        assert_eq!(bg, indexed_hex(1)); // red becomes the background
    }
}
