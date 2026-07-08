//! Reading and writing asciinema v2 cast files.
//!
//! Format: the first line is a JSON header object, every subsequent line is a
//! JSON array `[time, code, data]`. We only care about `"o"` (output) events
//! for rendering. See https://docs.asciinema.org/manual/asciicast/v2/

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::fs;
use std::io::Write;
use std::path::Path;

#[derive(Debug, Deserialize)]
struct Header {
    version: u32,
    width: usize,
    height: usize,
}

/// A single output event: seconds since start, plus the raw bytes written.
pub struct Event {
    pub time: f64,
    pub data: String,
}

/// A parsed cast: terminal size plus the ordered output stream.
pub struct Cast {
    pub width: usize,
    pub height: usize,
    pub events: Vec<Event>,
}

impl Cast {
    pub fn parse(path: &Path) -> Result<Cast> {
        let raw = fs::read_to_string(path)
            .with_context(|| format!("reading cast file {}", path.display()))?;
        let mut lines = raw.lines().filter(|l| !l.trim().is_empty());

        let header_line = lines.next().context("cast file is empty")?;
        let header: Header = serde_json::from_str(header_line)
            .context("first line is not a valid asciicast v2 header")?;
        if header.version != 2 {
            bail!(
                "only asciicast v2 is supported (got version {})",
                header.version
            );
        }

        let mut events = Vec::new();
        for (i, line) in lines.enumerate() {
            // Each event is a 3-tuple. Ignore input ("i") and marker ("m") events.
            let (time, code, data): (f64, String, String) = serde_json::from_str(line)
                .with_context(|| format!("event on line {} is malformed", i + 2))?;
            if code == "o" {
                events.push(Event { time, data });
            }
        }

        if events.is_empty() {
            bail!("cast file has no output events to render");
        }

        Ok(Cast {
            width: header.width,
            height: header.height,
            events,
        })
    }
}

/// Minimal asciicast v2 writer used by `castsvg record`.
pub struct Writer {
    file: fs::File,
}

impl Writer {
    pub fn create(path: &Path, width: usize, height: usize) -> Result<Writer> {
        let mut file = fs::File::create(path)
            .with_context(|| format!("creating cast file {}", path.display()))?;
        // Header. `timestamp` is optional in v2, so we omit it (the runtime has
        // no wall clock available anyway) and keep the record deterministic.
        writeln!(
            file,
            "{{\"version\":2,\"width\":{},\"height\":{},\"env\":{{\"TERM\":\"xterm-256color\"}}}}",
            width, height
        )?;
        Ok(Writer { file })
    }

    pub fn write_output(&mut self, time: f64, data: &[u8]) -> Result<()> {
        let text = String::from_utf8_lossy(data);
        let encoded = serde_json::to_string(&text.as_ref())?;
        writeln!(self.file, "[{:.6}, \"o\", {}]", time, encoded)?;
        Ok(())
    }
}
