#!/usr/bin/env node

import { createHash } from "node:crypto";
import {
  lstat,
  mkdir,
  open,
  readFile,
  readdir,
  rename,
  rm,
  stat,
  writeFile,
} from "node:fs/promises";
import { basename, dirname, join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const SIDECAR_PROTOCOL = "spock-asset-sidecar/1";
const PROTOCOLS = Object.freeze({
  environment: "spock-host-environment/1",
  project_status: "spock-project-status/1",
  project_event: "spock-project-event/1",
  editor_state: "uhura-editor-state/4",
  editor_event: "uhura-editor-event/0",
  ir: "uhura-ir/1",
  inspect: "uhura-inspection/0",
  view: "uhura-view/1",
  adapter_provider: "uhura-adapter-provider/0",
});
const REQUIRED_ROUTES = Object.freeze([
  "/api/editor/state",
  "/api/editor/events",
  "/api/play/events",
  "/api/play/config.json",
  "/api/play/ir.json",
  "/api/play/inspect.json",
  "/api/play/wasm/uhura_wasm.js",
]);
const REQUIRED_WASM = Object.freeze([
  "wasm/uhura_wasm.js",
  "wasm/uhura_wasm_bg.wasm",
]);
const COMMIT_PATTERN = /^[0-9a-f]{40}$/;
const HASH_PATTERN = /^[0-9a-f]{64}$/;
const PORTABLE_SEGMENT_PATTERN = /^[A-Za-z0-9][A-Za-z0-9._-]*$/;
const WINDOWS_DEVICE_PATTERN = /^(?:con|prn|aux|nul|com[1-9]|lpt[1-9])(?:\.|$)/i;

function fail(message) {
  throw new Error(`sidecar: ${message}`);
}

function usage() {
  return [
    "usage:",
    "  node npm/scripts/sidecar.mjs assemble \\",
    "    --web-dir uhura/web/dist \\",
    "    --wasm-dir uhura/crates/uhura-wasm/pkg/web \\",
    "    --out-dir npm/share/spock/uhura \\",
    "    --spock-commit <40-hex-sha> --uhura-commit <40-hex-sha>",
    "  node npm/scripts/sidecar.mjs verify --root npm/share/spock/uhura",
    "  node npm/scripts/sidecar.mjs self-test",
  ].join("\n");
}

function parseArgs(argv) {
  const [command, ...rest] = argv;
  if (command === "self-test") {
    if (rest.length !== 0) fail(usage());
    return { command, values: new Map() };
  }
  if (command !== "assemble" && command !== "verify") fail(usage());
  const values = new Map();
  for (let index = 0; index < rest.length; index += 2) {
    const key = rest[index];
    const value = rest[index + 1];
    if (!key?.startsWith("--") || value === undefined || value.startsWith("--")) {
      fail(usage());
    }
    if (values.has(key)) fail(`duplicate argument ${key}`);
    values.set(key, value);
  }
  const expected =
    command === "assemble"
      ? ["--web-dir", "--wasm-dir", "--out-dir", "--spock-commit", "--uhura-commit"]
      : ["--root"];
  for (const key of expected) {
    if (!values.has(key)) fail(`missing ${key}\n${usage()}`);
  }
  for (const key of values.keys()) {
    if (!expected.includes(key)) fail(`unknown argument ${key}\n${usage()}`);
  }
  return { command, values };
}

function portablePath(...parts) {
  return parts.filter(Boolean).join("/");
}

function comparePath(left, right) {
  return Buffer.compare(Buffer.from(left.path, "utf8"), Buffer.from(right.path, "utf8"));
}

function assertOutputDirectory(outDir) {
  const spockDir = dirname(outDir);
  const shareDir = dirname(spockDir);
  if (
    basename(outDir) !== "uhura" ||
    basename(spockDir) !== "spock" ||
    basename(shareDir) !== "share"
  ) {
    fail("--out-dir must end in share/spock/uhura");
  }
}

function assertAssetDirectory(root, rootStat) {
  if (rootStat?.isSymbolicLink()) fail(`asset directory may not be a symlink: ${root}`);
  if (!rootStat?.isDirectory()) fail(`asset directory is missing: ${root}`);
}

function selfTest() {
  const root = resolve("sidecar-output-suffix-self-test");
  assertOutputDirectory(join(root, "share", "spock", "uhura"));

  for (const invalid of [
    // Regression: checking only the last two segments accepted this spelling.
    join(root, "spock", "uhura"),
    join(root, "shared", "spock", "uhura"),
    join(root, "share", "other", "uhura"),
    join(root, "share", "spock", "other"),
    join(root, "share", "spock", "uhura", "extra"),
  ]) {
    let rejected = false;
    try {
      assertOutputDirectory(invalid);
    } catch (error) {
      rejected = error?.message === "sidecar: --out-dir must end in share/spock/uhura";
    }
    if (!rejected) fail(`self-test accepted invalid --out-dir: ${invalid}`);
  }

  const symlinkRoot = join(root, "symlink-root");
  try {
    assertAssetDirectory(symlinkRoot, {
      isSymbolicLink: () => true,
      isDirectory: () => false,
    });
    fail("self-test accepted a symlinked asset root");
  } catch (error) {
    if (error?.message !== `sidecar: asset directory may not be a symlink: ${symlinkRoot}`) {
      throw error;
    }
  }

  const missingRoot = join(root, "missing-root");
  try {
    assertAssetDirectory(missingRoot, null);
    fail("self-test accepted a missing asset root");
  } catch (error) {
    if (error?.message !== `sidecar: asset directory is missing: ${missingRoot}`) throw error;
  }

  process.stdout.write("verified sidecar tooling self-test\n");
}

async function regularFiles(root, prefix = "") {
  const rootStat = await lstat(root).catch(() => null);
  assertAssetDirectory(root, rootStat);

  const files = [];
  async function visit(directory, pathPrefix) {
    const entries = await readdir(directory, { withFileTypes: true });
    entries.sort((left, right) =>
      left.name < right.name ? -1 : left.name > right.name ? 1 : 0,
    );
    for (const entry of entries) {
      const absolute = join(directory, entry.name);
      const path = portablePath(pathPrefix, entry.name);
      if (entry.isSymbolicLink()) fail(`asset may not be a symlink: ${path}`);
      if (entry.isDirectory()) {
        await visit(absolute, path);
      } else if (entry.isFile()) {
        files.push({ absolute, path: portablePath(prefix, path) });
      } else {
        fail(`asset must be a regular file: ${path}`);
      }
    }
  }
  await visit(root, "");
  return files;
}

function assertPortableFileSet(files) {
  const folded = new Map();
  for (const { path } of files) {
    const segments = path.split("/");
    if (
      segments.length < 2 ||
      (segments[0] !== "web" && segments[0] !== "wasm") ||
      segments.some(
        (segment) =>
          !PORTABLE_SEGMENT_PATTERN.test(segment) ||
          segment.endsWith(".") ||
          WINDOWS_DEVICE_PATTERN.test(segment),
      )
    ) {
      fail(`manifest path is not portable ASCII: ${JSON.stringify(path)}`);
    }
    const key = path.toLowerCase();
    const previous = folded.get(key);
    if (previous !== undefined) {
      fail(`case-insensitive path collision: ${previous} and ${path}`);
    }
    folded.set(key, path);
  }
}

async function copyFile(source, destination) {
  const bytes = await readFile(source);
  await mkdir(dirname(destination), { recursive: true });
  await writeFile(destination, bytes, { flag: "wx" });
}

async function sha256(path) {
  const bytes = await readFile(path);
  return createHash("sha256").update(bytes).digest("hex");
}

async function manifestEntry(root, path) {
  const absolute = join(root, ...path.split("/"));
  const metadata = await stat(absolute);
  return { path, sha256: await sha256(absolute), size: metadata.size };
}

function assertProtocolMap(protocols) {
  if (typeof protocols !== "object" || protocols === null || Array.isArray(protocols)) {
    fail("protocols must be an object");
  }
  const actualKeys = Object.keys(protocols).sort();
  const expectedKeys = Object.keys(PROTOCOLS).sort();
  if (JSON.stringify(actualKeys) !== JSON.stringify(expectedKeys)) {
    fail(`protocol map keys differ from the ${SIDECAR_PROTOCOL} contract`);
  }
  for (const [name, version] of Object.entries(PROTOCOLS)) {
    if (protocols[name] !== version) fail(`protocol ${name} must be ${version}`);
  }
}

async function assertWebBundle(root) {
  const indexPath = join(root, "web", "index.html");
  const index = await readFile(indexPath, "utf8").catch(() => null);
  if (index === null) fail("web/index.html is missing");
  if (Buffer.byteLength(index) <= 200) fail("web/index.html is trivially small");
  if (!/<!doctype html>/i.test(index)) fail("web/index.html has no HTML doctype");

  const references = [...index.matchAll(/(?:src|href)=["']([^"']+)["']/gi)].map(
    (match) => match[1],
  );
  const localAssets = references
    .filter((value) => !/^(?:[a-z][a-z0-9+.-]*:|\/|#)/i.test(value))
    .map((value) => decodeURIComponent(value.split(/[?#]/, 1)[0]))
    .filter((value) => /\.(?:css|m?js)$/i.test(value));
  const rootedAssets = references
    .filter((value) => value.startsWith("/") && !value.startsWith("//"))
    .map((value) => decodeURIComponent(value.split(/[?#]/, 1)[0]).slice(1))
    .filter((value) => /\.(?:css|m?js)$/i.test(value));
  const assets = [...new Set([...localAssets, ...rootedAssets])];
  if (assets.length === 0) fail("web/index.html references no local JavaScript or CSS");
  for (const asset of assets) {
    if (asset.includes("\\") || asset.split("/").some((part) => part === "..")) {
      fail(`web/index.html contains an unsafe asset reference: ${asset}`);
    }
    const metadata = await stat(join(root, "web", ...asset.split("/"))).catch(() => null);
    if (!metadata?.isFile() || metadata.size === 0) {
      fail(`web/index.html references missing or empty asset: ${asset}`);
    }
  }

  const webFiles = await regularFiles(join(root, "web"));
  const scripts = webFiles
    .filter(({ path }) => /\.m?js$/i.test(path))
    .sort(comparePath);
  const scriptText = (await Promise.all(scripts.map(({ absolute }) => readFile(absolute, "utf8")))).join(
    "\n",
  );
  for (const route of REQUIRED_ROUTES) {
    if (!scriptText.includes(route)) fail(`Uhura web build does not reference route ${route}`);
  }
}

async function assertWasmBundle(root) {
  for (const path of REQUIRED_WASM) {
    const metadata = await stat(join(root, ...path.split("/"))).catch(() => null);
    if (!metadata?.isFile() || metadata.size === 0) fail(`required artifact is missing: ${path}`);
  }
  const module = await open(join(root, "wasm", "uhura_wasm_bg.wasm"), "r");
  try {
    const magic = Buffer.alloc(4);
    const { bytesRead } = await module.read(magic, 0, magic.length, 0);
    if (bytesRead !== 4 || !magic.equals(Buffer.from([0x00, 0x61, 0x73, 0x6d]))) {
      fail("wasm/uhura_wasm_bg.wasm has invalid WebAssembly magic");
    }
  } finally {
    await module.close();
  }
  const glue = await readFile(join(root, "wasm", "uhura_wasm.js"), "utf8");
  if (!glue.includes("uhura_wasm_bg.wasm")) {
    fail("wasm/uhura_wasm.js does not load uhura_wasm_bg.wasm");
  }
}

async function verify(rootArg) {
  const root = resolve(rootArg);
  const rawManifest = await readFile(join(root, "manifest.json"), "utf8").catch(() => null);
  if (rawManifest === null) fail(`manifest is missing under ${root}`);
  let manifest;
  try {
    manifest = JSON.parse(rawManifest);
  } catch (error) {
    fail(`manifest is not valid JSON: ${error.message}`);
  }
  if (manifest.protocol !== SIDECAR_PROTOCOL) {
    fail(`manifest protocol must be ${SIDECAR_PROTOCOL}`);
  }
  const topLevelKeys = Object.keys(manifest).sort();
  const expectedTopLevelKeys = ["files", "protocol", "protocols", "spock_commit", "uhura_commit"];
  if (JSON.stringify(topLevelKeys) !== JSON.stringify(expectedTopLevelKeys)) {
    fail("manifest top-level keys differ from the sidecar contract");
  }
  if (!COMMIT_PATTERN.test(manifest.spock_commit ?? "")) fail("invalid spock_commit");
  if (!COMMIT_PATTERN.test(manifest.uhura_commit ?? "")) fail("invalid uhura_commit");
  assertProtocolMap(manifest.protocols);
  if (!Array.isArray(manifest.files) || manifest.files.length === 0) {
    fail("manifest files must be a non-empty array");
  }

  const actualFiles = [
    ...(await regularFiles(join(root, "web"), "web")),
    ...(await regularFiles(join(root, "wasm"), "wasm")),
  ].sort(comparePath);
  assertPortableFileSet(actualFiles);
  const expectedPaths = actualFiles.map(({ path }) => path);
  const manifestPaths = manifest.files.map((entry) => entry?.path);
  if (JSON.stringify(manifestPaths) !== JSON.stringify(expectedPaths)) {
    fail("manifest file inventory is missing, extra, duplicated, or not sorted");
  }

  for (const entry of manifest.files) {
    if (
      typeof entry !== "object" ||
      entry === null ||
      Array.isArray(entry) ||
      JSON.stringify(Object.keys(entry).sort()) !== JSON.stringify(["path", "sha256", "size"])
    ) {
      fail("each manifest file must contain exactly path, sha256, and size");
    }
    if (!Number.isSafeInteger(entry.size) || entry.size <= 0) {
      fail(`invalid size for ${entry.path}`);
    }
    if (!HASH_PATTERN.test(entry.sha256 ?? "")) fail(`invalid sha256 for ${entry.path}`);
    const actual = await manifestEntry(root, entry.path);
    if (actual.size !== entry.size) fail(`size mismatch for ${entry.path}`);
    if (actual.sha256 !== entry.sha256) fail(`sha256 mismatch for ${entry.path}`);
  }
  await assertWebBundle(root);
  await assertWasmBundle(root);
  const total = manifest.files.reduce((sum, entry) => sum + entry.size, 0);
  process.stdout.write(
    `verified ${SIDECAR_PROTOCOL}: ${manifest.files.length} files, ${total} bytes\n`,
  );
}

async function assemble(values) {
  const webDir = resolve(values.get("--web-dir"));
  const wasmDir = resolve(values.get("--wasm-dir"));
  const outDir = resolve(values.get("--out-dir"));
  const spockCommit = values.get("--spock-commit");
  const uhuraCommit = values.get("--uhura-commit");
  if (!COMMIT_PATTERN.test(spockCommit)) fail("--spock-commit must be a 40-character hex SHA");
  if (!COMMIT_PATTERN.test(uhuraCommit)) fail("--uhura-commit must be a 40-character hex SHA");
  assertOutputDirectory(outDir);
  for (const source of [webDir, wasmDir]) {
    const fromOutput = relative(outDir, source);
    if (fromOutput === "" || (!fromOutput.startsWith(`..${sep}`) && fromOutput !== "..")) {
      fail("source directories may not be inside the output directory");
    }
  }

  const sourceFiles = [
    ...(await regularFiles(webDir, "web")),
    ...(await regularFiles(wasmDir, "wasm")),
  ].sort(comparePath);
  if (sourceFiles.length === 0) fail("source asset trees are empty");
  assertPortableFileSet(sourceFiles);

  await mkdir(dirname(outDir), { recursive: true });
  const staging = `${outDir}.tmp-${process.pid}-${Date.now()}`;
  await rm(staging, { recursive: true, force: true });
  try {
    await mkdir(staging, { recursive: false });
    for (const file of sourceFiles) {
      await copyFile(file.absolute, join(staging, ...file.path.split("/")));
    }
    const files = [];
    for (const file of sourceFiles) files.push(await manifestEntry(staging, file.path));
    const manifest = {
      protocol: SIDECAR_PROTOCOL,
      spock_commit: spockCommit,
      uhura_commit: uhuraCommit,
      protocols: PROTOCOLS,
      files,
    };
    await writeFile(join(staging, "manifest.json"), `${JSON.stringify(manifest, null, 2)}\n`, {
      flag: "wx",
    });
    await verify(staging);
    await rm(outDir, { recursive: true, force: true });
    await rename(staging, outDir);
  } catch (error) {
    await rm(staging, { recursive: true, force: true });
    throw error;
  }
  process.stdout.write(`assembled ${outDir}\n`);
}

async function main() {
  const { command, values } = parseArgs(process.argv.slice(2));
  if (command === "assemble") {
    await assemble(values);
  } else if (command === "verify") {
    await verify(values.get("--root"));
  } else {
    selfTest();
  }
}

// Avoid executing when imported by a future test harness.
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main().catch((error) => {
    process.stderr.write(`${error.message}\n`);
    process.exitCode = 1;
  });
}
