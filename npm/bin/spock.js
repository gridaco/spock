#!/usr/bin/env node
// The `spock` npm package bundles one prebuilt binary per platform (RFD 0020).
// This shim resolves the binary matching the host's os+arch and hands off argv
// verbatim. No network, no postinstall — the binary is already on disk under
// binaries/<platform>-<arch>/. Detecting os+arch at runtime (rather than
// trusting which package installed) is the robust path across npm/pnpm/yarn/bun.
"use strict";

const { execFileSync } = require("node:child_process");
const path = require("node:path");
const fs = require("node:fs");

const key = `${process.platform}-${process.arch}`;
const exe = process.platform === "win32" ? "spock.exe" : "spock";
const bin = path.join(__dirname, "..", "binaries", key, exe);

if (!fs.existsSync(bin)) {
  process.stderr.write(
    `spock: no prebuilt binary for ${key}.\n` +
      `Supported platforms: darwin-arm64, darwin-x64, linux-x64, win32-x64.\n` +
      `Please open an issue at https://github.com/gridaco/spock/issues.\n`,
  );
  process.exit(1);
}

try {
  execFileSync(bin, process.argv.slice(2), { stdio: "inherit" });
} catch (err) {
  // Propagate the child's exit code; a non-numeric status means a spawn error.
  if (typeof err.status === "number") process.exit(err.status);
  process.stderr.write(`spock: failed to run binary: ${err.message}\n`);
  process.exit(1);
}
