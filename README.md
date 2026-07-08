# castsvg

**Record a terminal session, get a self-contained animated SVG.** One static Rust binary — no Node, no Python, no runtime, no external player. The output is a plain `.svg` that loops forever and drops straight into a README, a docs page, or an email.

<p align="center">
  <img src="examples/demo.svg" alt="castsvg demo" width="640">
</p>

> The animation above *is* an SVG produced by castsvg. No GIF, no JS, no `<video>` — just `<text>` and CSS `@keyframes`.

## Why

`termtosvg` needs Python. `svg-term-cli` needs Node. `asciinema` needs a JS player hosted somewhere (or its own server) to actually see the recording. If you just want a lightweight, self-contained animation to paste into a README, all of those drag a runtime along.

`castsvg` is a single binary that does two things:

- **`record`** — run a shell (or a command) inside a real PTY and capture it to an asciicast v2 `.cast` file.
- **`render`** — turn any asciicast v2 `.cast` into an animated SVG.

Because the recorder writes standard asciicast v2, it also renders files produced by `asciinema rec`.

## Install

Pick whichever fits — all three give you the **same single binary**:

**Download a prebuilt binary** (no toolchain needed) from the [Releases](https://github.com/q6r/castsvg/releases) page — macOS (Intel/Apple Silicon), Linux, Windows.

**npm** (for Node users — downloads the prebuilt binary on install, no Rust required):

```sh
npm install -g castsvg
```

**Cargo** (builds from source; needs the Rust toolchain):

```sh
cargo install castsvg
# or from a clone:
git clone https://github.com/q6r/castsvg && cd castsvg && cargo install --path .
```

The release profile builds a stripped, LTO'd single binary (~500 KB, no dynamic runtime).

## Usage

Record a session, then render it:

```sh
castsvg record session.cast          # records your $SHELL until you exit
castsvg render session.cast -o out.svg
```

Record a single command instead of an interactive shell:

```sh
castsvg record build.cast --command "cargo build --release"
```

Render an existing asciinema recording:

```sh
castsvg render demo.cast -o demo.svg --theme light --font-size 16
```

### render options

| flag | default | meaning |
|------|---------|---------|
| `-o, --output` | `out.svg` | output SVG path |
| `--theme` | `dark` | `dark` or `light` |
| `--font-size` | `14` | glyph size in px |
| `--min-frame-ms` | `40` | coalesce output bursts closer than this |
| `--idle-cap-ms` | `1000` | clamp long pauses so dead air doesn't bloat the timeline |
| `--end-pause-ms` | `1500` | hold the final frame before looping |
| `--no-loop` | off | play once and stop on the last frame |

## How it works

1. **`record`** spawns your command in a real pseudo-terminal (ConPTY on Windows, a Unix PTY elsewhere via `portable-pty`), mirrors output to your screen so the session stays fully interactive, and timestamps every write into asciicast v2.
2. **`render`** replays that byte stream through a small VT100 emulator (built on [`vte`](https://crates.io/crates/vte), the parser Alacritty uses), snapshotting the character grid along the timeline.
3. Each snapshot becomes an SVG `<g>` layer; CSS `@keyframes` flip layer opacity so exactly one frame shows at a time. Identical-colour runs are merged into single `<text>`/`<rect>` elements to keep the file small.

The result needs nothing but an SVG renderer — i.e. any browser, or GitHub's own Markdown.

## Limitations

Honest about scope — it renders the common cases well, not every escape sequence:

- 16 / 256 / truecolor SGR, bold, and inverse are supported. Underline, italics, and blink are not (yet).
- The alternate screen buffer (full-screen TUIs like `vim`, `htop`) is treated as the normal buffer — line-oriented output is the sweet spot.
- Cursor is not drawn as a blinking block (a static inverse cell works via `\e[7m`).

PRs for any of these are welcome.

## License

MIT
