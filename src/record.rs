//! Record a live terminal session into an asciicast v2 file.
//!
//! We spawn the target command inside a real PTY (ConPTY on Windows, a Unix PTY
//! elsewhere via `portable-pty`), mirror its output to our own stdout so the
//! session stays interactive, and log every write with a timestamp.

use crate::cast;
use anyhow::{Context, Result};
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::{Read, Write};
use std::path::Path;
use std::time::Instant;

fn build_command(command: Option<&str>) -> CommandBuilder {
    let mut cmd = match command {
        Some(c) => {
            if cfg!(windows) {
                let mut b = CommandBuilder::new("cmd.exe");
                b.arg("/C");
                b.arg(c);
                b
            } else {
                let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
                let mut b = CommandBuilder::new(shell);
                b.arg("-c");
                b.arg(c);
                b
            }
        }
        None => {
            if cfg!(windows) {
                CommandBuilder::new(std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".into()))
            } else {
                CommandBuilder::new(std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into()))
            }
        }
    };
    cmd.env("TERM", "xterm-256color");
    if let Ok(dir) = std::env::current_dir() {
        cmd.cwd(dir);
    }
    cmd
}

pub fn run(output: &Path, command: Option<&str>) -> Result<()> {
    let (cols, rows) = crossterm::terminal::size().unwrap_or((80, 24));

    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .context("failed to open PTY")?;

    let mut child = pair
        .slave
        .spawn_command(build_command(command))
        .context("failed to spawn command")?;
    // Slave handle is held by the child now; drop ours so EOF propagates on exit.
    drop(pair.slave);

    let mut reader = pair
        .master
        .try_clone_reader()
        .context("cloning PTY reader")?;
    let mut writer = pair.master.take_writer().context("taking PTY writer")?;

    let mut cast_writer = cast::Writer::create(output, cols as usize, rows as usize)?;
    let start = Instant::now();

    crossterm::terminal::enable_raw_mode().ok();
    eprintln!(
        "castsvg: recording to {} — exit the shell to stop.",
        output.display()
    );

    // Forward our stdin to the PTY. Detached: it may block on read() when the
    // child exits, so we never join it — the process exits and takes it down.
    std::thread::spawn(move || {
        let mut stdin = std::io::stdin();
        let mut buf = [0u8; 4096];
        loop {
            match stdin.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if writer.write_all(&buf[..n]).is_err() {
                        break;
                    }
                    let _ = writer.flush();
                }
            }
        }
    });

    // Pump PTY output to our stdout and the cast log until EOF.
    let reader_thread = std::thread::spawn(move || -> Result<()> {
        let mut stdout = std::io::stdout();
        let mut buf = [0u8; 8192];
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let t = start.elapsed().as_secs_f64();
                    cast_writer.write_output(t, &buf[..n])?;
                    stdout.write_all(&buf[..n]).ok();
                    stdout.flush().ok();
                }
            }
        }
        Ok(())
    });

    let status = child.wait().context("waiting for child")?;
    // Give the reader a moment to drain, then tear down.
    let _ = reader_thread.join();
    crossterm::terminal::disable_raw_mode().ok();

    eprintln!(
        "\ncastsvg: saved {} (exit status {:?}). Render it with: castsvg render {}",
        output.display(),
        status.exit_code(),
        output.display()
    );
    Ok(())
}
