# castsvg (npm)

Record a terminal session, get a self-contained **animated SVG**. One static binary — this npm package just downloads the prebuilt [castsvg](https://github.com/YOUR_USER/castsvg) binary (written in Rust) for your platform. No Node runtime is bundled into the output; the SVG needs nothing but a browser.

```sh
npm install -g castsvg
```

Then:

```sh
castsvg record session.cast          # record your shell until you exit
castsvg render session.cast -o out.svg
```

Or record a single command:

```sh
castsvg record build.cast --command "npm run build"
castsvg render build.cast -o build.svg --theme light
```

The resulting `.svg` loops forever, has no `<script>`, and drops straight into a README or docs page.

## How this package works

On install, a postinstall script downloads the matching prebuilt binary from the project's GitHub Releases (`x86_64`/`arm64` for macOS, `x86_64` for Linux and Windows). It's the **same binary** you'd get from `cargo install castsvg` — the npm package is just a convenience wrapper so Node users don't need the Rust toolchain.

If your platform has no prebuilt binary, install Rust and run `cargo install castsvg` instead.

Full documentation and source: **https://github.com/YOUR_USER/castsvg**

## License

MIT
