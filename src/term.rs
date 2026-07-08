//! A small VT100-subset terminal emulator.
//!
//! We feed the raw output byte stream through `vte` (the parser Alacritty uses)
//! and maintain a grid of cells. Snapshots of that grid, taken along the cast's
//! timeline, become the animation frames.

use crate::cast::Cast;
use vte::{Params, Parser, Perform};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Color {
    Default,
    Indexed(u8),
    Rgb(u8, u8, u8),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    pub ch: char,
    pub fg: Color,
    pub bg: Color,
    pub bold: bool,
    pub inverse: bool,
}

impl Default for Cell {
    fn default() -> Self {
        Cell {
            ch: ' ',
            fg: Color::Default,
            bg: Color::Default,
            bold: false,
            inverse: false,
        }
    }
}

/// One animation frame: a full grid snapshot plus how long it stays on screen.
pub struct Frame {
    pub cells: Vec<Cell>,
    pub duration_ms: f64,
}

/// The full render model handed to the SVG backend.
pub struct Model {
    pub cols: usize,
    pub rows: usize,
    pub frames: Vec<Frame>,
}

struct Grid {
    cols: usize,
    rows: usize,
    cells: Vec<Cell>,
    cx: usize,
    cy: usize,
    // Current pen attributes (the `ch` field is unused here).
    fg: Color,
    bg: Color,
    bold: bool,
    inverse: bool,
}

impl Grid {
    fn new(cols: usize, rows: usize) -> Grid {
        Grid {
            cols,
            rows,
            cells: vec![Cell::default(); cols * rows],
            cx: 0,
            cy: 0,
            fg: Color::Default,
            bg: Color::Default,
            bold: false,
            inverse: false,
        }
    }

    fn blank(&self) -> Cell {
        // Erased cells keep the current background so full-screen colours render.
        Cell {
            ch: ' ',
            fg: Color::Default,
            bg: self.bg,
            bold: false,
            inverse: false,
        }
    }

    fn linefeed(&mut self) {
        self.cy += 1;
        if self.cy >= self.rows {
            // Scroll up one line.
            self.cells.drain(0..self.cols);
            let blank = self.blank();
            self.cells.extend(std::iter::repeat(blank).take(self.cols));
            self.cy = self.rows - 1;
        }
    }

    fn erase_range(&mut self, start: usize, end: usize) {
        let blank = self.blank();
        let end = end.min(self.cells.len());
        for cell in &mut self.cells[start..end] {
            *cell = blank;
        }
    }

    fn snapshot(&self) -> Vec<Cell> {
        self.cells.clone()
    }
}

/// Read the primary value of the nth CSI parameter, treating 0/absent as `def`.
fn param(params: &Params, idx: usize, def: usize) -> usize {
    params
        .iter()
        .nth(idx)
        .and_then(|s| s.first().copied())
        .filter(|&v| v != 0)
        .map(|v| v as usize)
        .unwrap_or(def)
}

impl Perform for Grid {
    fn print(&mut self, c: char) {
        if self.cx >= self.cols {
            self.cx = 0;
            self.linefeed();
        }
        let idx = self.cy * self.cols + self.cx;
        if idx < self.cells.len() {
            self.cells[idx] = Cell {
                ch: c,
                fg: self.fg,
                bg: self.bg,
                bold: self.bold,
                inverse: self.inverse,
            };
        }
        self.cx += 1;
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            b'\n' => self.linefeed(),
            b'\r' => self.cx = 0,
            0x08 => {
                if self.cx > 0 {
                    self.cx -= 1;
                }
            }
            b'\t' => {
                self.cx = ((self.cx / 8) + 1) * 8;
                if self.cx >= self.cols {
                    self.cx = self.cols - 1;
                }
            }
            _ => {}
        }
    }

    fn csi_dispatch(&mut self, params: &Params, intermediates: &[u8], _ignore: bool, action: char) {
        // Ignore private-mode sequences (e.g. `?25l` hide cursor, alt screen).
        if intermediates.first() == Some(&b'?') {
            return;
        }
        match action {
            'H' | 'f' => {
                self.cy = (param(params, 0, 1) - 1).min(self.rows - 1);
                self.cx = (param(params, 1, 1) - 1).min(self.cols - 1);
            }
            'A' => self.cy = self.cy.saturating_sub(param(params, 0, 1)),
            'B' => self.cy = (self.cy + param(params, 0, 1)).min(self.rows - 1),
            'C' => self.cx = (self.cx + param(params, 0, 1)).min(self.cols - 1),
            'D' => self.cx = self.cx.saturating_sub(param(params, 0, 1)),
            'G' => self.cx = (param(params, 0, 1) - 1).min(self.cols - 1),
            'd' => self.cy = (param(params, 0, 1) - 1).min(self.rows - 1),
            'J' => {
                let mode = param(params, 0, 0);
                let cur = self.cy * self.cols + self.cx;
                match mode {
                    0 => self.erase_range(cur, self.cells.len()),
                    1 => self.erase_range(0, cur + 1),
                    _ => self.erase_range(0, self.cells.len()),
                }
            }
            'K' => {
                let mode = param(params, 0, 0);
                let row_start = self.cy * self.cols;
                let cur = row_start + self.cx;
                let row_end = row_start + self.cols;
                match mode {
                    0 => self.erase_range(cur, row_end),
                    1 => self.erase_range(row_start, cur + 1),
                    _ => self.erase_range(row_start, row_end),
                }
            }
            'm' => self.sgr(params),
            _ => {}
        }
    }
}

impl Grid {
    fn sgr(&mut self, params: &Params) {
        // Flatten params and sub-params so both `38;5;1` and `38:5:1` work.
        let mut flat: Vec<u16> = Vec::new();
        for sub in params.iter() {
            for &v in sub {
                flat.push(v);
            }
        }
        if flat.is_empty() {
            flat.push(0);
        }

        let mut i = 0;
        while i < flat.len() {
            match flat[i] {
                0 => {
                    self.fg = Color::Default;
                    self.bg = Color::Default;
                    self.bold = false;
                    self.inverse = false;
                }
                1 => self.bold = true,
                22 => self.bold = false,
                7 => self.inverse = true,
                27 => self.inverse = false,
                30..=37 => self.fg = Color::Indexed((flat[i] - 30) as u8),
                39 => self.fg = Color::Default,
                40..=47 => self.bg = Color::Indexed((flat[i] - 40) as u8),
                49 => self.bg = Color::Default,
                90..=97 => self.fg = Color::Indexed((flat[i] - 90 + 8) as u8),
                100..=107 => self.bg = Color::Indexed((flat[i] - 100 + 8) as u8),
                38 | 48 => {
                    let is_fg = flat[i] == 38;
                    match flat.get(i + 1) {
                        Some(5) => {
                            if let Some(&n) = flat.get(i + 2) {
                                let c = Color::Indexed(n as u8);
                                if is_fg {
                                    self.fg = c;
                                } else {
                                    self.bg = c;
                                }
                            }
                            i += 2;
                        }
                        Some(2) => {
                            if let (Some(&r), Some(&g), Some(&b)) =
                                (flat.get(i + 2), flat.get(i + 3), flat.get(i + 4))
                            {
                                let c = Color::Rgb(r as u8, g as u8, b as u8);
                                if is_fg {
                                    self.fg = c;
                                } else {
                                    self.bg = c;
                                }
                            }
                            i += 4;
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
            i += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Feed raw bytes through the emulator and return the final grid.
    fn run(cols: usize, rows: usize, bytes: &[u8]) -> Grid {
        let mut grid = Grid::new(cols, rows);
        let mut parser = Parser::new();
        for &b in bytes {
            parser.advance(&mut grid, b);
        }
        grid
    }

    fn text_at(grid: &Grid, row: usize) -> String {
        (0..grid.cols)
            .map(|c| grid.cells[row * grid.cols + c].ch)
            .collect::<String>()
            .trim_end()
            .to_string()
    }

    #[test]
    fn plain_text_lands_on_the_grid() {
        let g = run(10, 2, b"hi");
        assert_eq!(text_at(&g, 0), "hi");
    }

    #[test]
    fn carriage_return_overwrites() {
        // A progress-bar style redraw: "aaaa\rbb" leaves "bbaa".
        let g = run(10, 1, b"aaaa\rbb");
        assert_eq!(text_at(&g, 0), "bbaa");
    }

    #[test]
    fn sgr_sets_foreground() {
        let g = run(10, 1, b"\x1b[31mR");
        assert_eq!(g.cells[0].ch, 'R');
        assert_eq!(g.cells[0].fg, Color::Indexed(1));
    }

    #[test]
    fn truecolor_fg_is_parsed() {
        let g = run(10, 1, b"\x1b[38;2;10;20;30mX");
        assert_eq!(g.cells[0].fg, Color::Rgb(10, 20, 30));
    }

    #[test]
    fn erase_line_clears_to_end() {
        let g = run(10, 1, b"abcdef\r\x1b[3C\x1b[K");
        // cursor moved to col 3, erase-to-end wipes "def".
        assert_eq!(text_at(&g, 0), "abc");
    }

    #[test]
    fn linefeed_scrolls_when_past_bottom() {
        let g = run(4, 2, b"top\r\nmid\r\nbot");
        // Only two rows: "top" scrolled off, leaving "mid" then "bot".
        assert_eq!(text_at(&g, 0), "mid");
        assert_eq!(text_at(&g, 1), "bot");
    }
}

/// Replay a cast and slice it into animation frames.
///
/// * `min_frame_ms` coalesces bursts of output that are closer together than a
///   single frame — no point emitting 500 near-identical frames for a fast
///   `cat`.
/// * `idle_cap_ms` compresses long pauses so a recording where you paused to
///   think doesn't produce a 30-second dead SVG.
pub fn build_model(cast: &Cast, min_frame_ms: f64, idle_cap_ms: f64, end_pause_ms: f64) -> Model {
    let mut grid = Grid::new(cast.width, cast.height);
    let mut parser = Parser::new();

    // Pass 1: replay, taking a snapshot after each event on a remapped clock.
    let mut snaps: Vec<(f64, Vec<Cell>)> = vec![(0.0, grid.snapshot())];
    let mut clock = 0.0_f64;
    let mut last_real = 0.0_f64;
    for ev in &cast.events {
        let delta = (ev.time - last_real).max(0.0);
        last_real = ev.time;
        clock += delta.min(idle_cap_ms / 1000.0);
        for &byte in ev.data.as_bytes() {
            parser.advance(&mut grid, byte);
        }
        snaps.push((clock * 1000.0, grid.snapshot()));
    }

    // Pass 2: coalesce snapshots closer than min_frame_ms into one frame.
    let mut frames: Vec<Frame> = Vec::new();
    let mut frame_start = snaps[0].0;
    let mut current = snaps[0].1.clone();
    for (t, cells) in snaps.iter().skip(1) {
        if t - frame_start >= min_frame_ms {
            frames.push(Frame {
                cells: std::mem::replace(&mut current, cells.clone()),
                duration_ms: t - frame_start,
            });
            frame_start = *t;
        } else {
            current = cells.clone();
        }
    }
    // Final frame is held for a beat so the last state is readable before looping.
    frames.push(Frame {
        cells: current,
        duration_ms: end_pause_ms.max(min_frame_ms),
    });

    Model {
        cols: cast.width,
        rows: cast.height,
        frames,
    }
}
