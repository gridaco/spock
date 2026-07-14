#!/usr/bin/env node
// The `spock` npm package bundles one prebuilt binary per platform (RFD 0020).
// This shim resolves the binary matching the host's os+arch and hands off argv
// verbatim. No network, no postinstall — the binary is already on disk under
// binaries/<platform>-<arch>/. Detecting os+arch at runtime (rather than
// trusting which package installed) is the robust path across npm/pnpm/yarn/bun.
"use strict";

const { spawn } = require("node:child_process");
const { constants } = require("node:os");
const path = require("node:path");
const fs = require("node:fs");

const key = `${process.platform}-${process.arch}`;
const exe = process.platform === "win32" ? "spock.exe" : "spock";
const bin = path.join(__dirname, "..", "binaries", key, exe);

if (key === "linux-x64") {
  let nonGlibcRuntime = false;
  try {
    const report = process.report?.getReport?.();
    nonGlibcRuntime =
      report !== undefined &&
      report !== null &&
      typeof report.header?.glibcVersionRuntime !== "string";
  } catch {
    // If Node cannot report libc, let the native loader provide the diagnosis.
  }
  if (nonGlibcRuntime) {
    process.stderr.write(
      "spock: the bundled linux-x64 binary requires GNU libc.\n" +
        "Alpine and other musl-based distributions are not supported yet; " +
        "use a GNU-libc image or build Spock from source.\n",
    );
    process.exit(1);
  }
}

if (!fs.existsSync(bin)) {
  process.stderr.write(
    `spock: no prebuilt binary for ${key}.\n` +
      `Supported platforms: darwin-arm64, darwin-x64, linux-x64, win32-x64.\n` +
      `Please open an issue at https://github.com/gridaco/spock/issues.\n`,
  );
  process.exit(1);
}

const child = spawn(bin, process.argv.slice(2), { stdio: "inherit" });
let spawnError;
let forwardingError;
let spawned = false;
let pendingSignal;

// `start`, `dev`, and `run` are long-lived. A signal sent specifically to the
// npm shim (as opposed to the whole terminal process group) must still reach
// the Rust owner so it can release its listener, locks, and background tasks.
const forwardedSignals = ["SIGINT", "SIGTERM", "SIGHUP"];
const handlers = new Map();
for (const signal of forwardedSignals) {
  const handler = () => {
    if (child.exitCode !== null || child.signalCode !== null) return;
    try {
      pendingSignal = signal;
      child.kill(signal);
    } catch (error) {
      // The child may have exited between the state check and kill(2).
      if (error?.code !== "ESRCH" && forwardingError === undefined) {
        forwardingError = { signal, error };
      }
    }
  };
  try {
    process.on(signal, handler);
    handlers.set(signal, handler);
  } catch {
    // Some signals are unavailable on some Node/Windows combinations.
  }
}

child.once("spawn", () => {
  spawned = true;
});

child.on("error", (error) => {
  if (!spawned) {
    spawnError ??= error;
  } else if (error?.code !== "ESRCH" && forwardingError === undefined) {
    forwardingError = { signal: pendingSignal ?? "signal", error };
  }
});

child.once("close", (code, signal) => {
  for (const [name, handler] of handlers) process.removeListener(name, handler);
  if (spawnError) {
    process.stderr.write(`spock: failed to run binary: ${spawnError.message}\n`);
    process.exitCode = 1;
    return;
  }
  if (forwardingError) {
    const details =
      forwardingError.error instanceof Error
        ? forwardingError.error.message
        : String(forwardingError.error);
    process.stderr.write(
      `spock: failed to forward ${forwardingError.signal}: ${details}\n`,
    );
  }
  if (signal) {
    const number = signal ? constants.signals[signal] : undefined;
    process.exitCode = typeof number === "number" ? 128 + number : 1;
    // On Unix, preserve true signal termination for shells and process
    // supervisors. Windows and unsupported Node signal combinations retain
    // the conventional 128+signal fallback above.
    if (process.platform !== "win32") {
      setImmediate(() => {
        try {
          process.kill(process.pid, signal);
        } catch {
          // The fallback exit code is already installed.
        }
      });
    }
  } else if (forwardingError) {
    process.exitCode = 1;
  } else if (typeof code === "number") {
    process.exitCode = code;
  } else {
    process.exitCode = 1;
  }
});
