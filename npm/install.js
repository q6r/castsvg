#!/usr/bin/env node
// Postinstall step: download the prebuilt castsvg binary for this platform from
// the matching GitHub release. We publish RAW binaries (no tar/zip) so this stays
// dependency-free — no archive extraction, no npm deps to audit.

const fs = require("fs");
const path = require("path");
const https = require("https");
const { version } = require("./package.json");

const REPO = "q6r/castsvg";

// process.platform + process.arch -> Rust target triple used in the asset name.
const TARGETS = {
  "linux-x64": "x86_64-unknown-linux-gnu",
  "darwin-x64": "x86_64-apple-darwin",
  "darwin-arm64": "aarch64-apple-darwin",
  "win32-x64": "x86_64-pc-windows-msvc",
};

function selectAsset() {
  const key = `${process.platform}-${process.arch}`;
  const target = TARGETS[key];
  if (!target) {
    console.error(`castsvg: no prebuilt binary for ${key}.`);
    console.error("Install the Rust toolchain and run: cargo install castsvg");
    // Not a hard failure — the platform simply isn't covered by a prebuilt.
    process.exit(0);
  }
  const ext = process.platform === "win32" ? ".exe" : "";
  return { asset: `castsvg-${target}${ext}`, ext };
}

// GitHub release download URLs redirect (to objects.githubusercontent.com),
// so follow 3xx responses manually.
function download(url, dest, redirects = 0) {
  return new Promise((resolve, reject) => {
    if (redirects > 10) return reject(new Error("too many redirects"));
    https
      .get(url, { headers: { "User-Agent": "castsvg-npm-installer" } }, (res) => {
        if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
          res.resume();
          return resolve(download(res.headers.location, dest, redirects + 1));
        }
        if (res.statusCode !== 200) {
          res.resume();
          return reject(new Error(`HTTP ${res.statusCode}`));
        }
        const file = fs.createWriteStream(dest);
        res.pipe(file);
        file.on("finish", () => file.close(() => resolve()));
        file.on("error", reject);
      })
      .on("error", reject);
  });
}

async function main() {
  // Escape hatch for CI / offline / source builds.
  if (process.env.CASTSVG_SKIP_DOWNLOAD) {
    console.log("castsvg: CASTSVG_SKIP_DOWNLOAD set, skipping binary download.");
    return;
  }

  const { asset, ext } = selectAsset();
  const outDir = path.join(__dirname, "binary");
  fs.mkdirSync(outDir, { recursive: true });
  const dest = path.join(outDir, `castsvg${ext}`);
  const url = `https://github.com/${REPO}/releases/download/v${version}/${asset}`;

  console.log(`castsvg: downloading ${asset} (v${version})...`);
  try {
    await download(url, dest);
    if (process.platform !== "win32") fs.chmodSync(dest, 0o755);
    console.log("castsvg: binary installed.");
  } catch (err) {
    console.error(`castsvg: failed to download binary: ${err.message}`);
    console.error(`  expected: ${url}`);
    console.error("  fallback: install Rust and run `cargo install castsvg`");
    process.exit(1);
  }
}

main();
