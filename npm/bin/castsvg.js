#!/usr/bin/env node
// Thin launcher: exec the downloaded native binary, forwarding args and stdio.

const { spawnSync } = require("child_process");
const path = require("path");
const fs = require("fs");

const ext = process.platform === "win32" ? ".exe" : "";
const bin = path.join(__dirname, "..", "binary", `castsvg${ext}`);

if (!fs.existsSync(bin)) {
  console.error("castsvg: native binary not found — the postinstall download may have failed.");
  console.error("Try reinstalling the package, or build from source: cargo install castsvg");
  process.exit(1);
}

const result = spawnSync(bin, process.argv.slice(2), { stdio: "inherit" });
if (result.error) {
  console.error(`castsvg: failed to run binary: ${result.error.message}`);
  process.exit(1);
}
// Mirror the child's exit code (null => killed by signal).
process.exit(result.status === null ? 1 : result.status);
