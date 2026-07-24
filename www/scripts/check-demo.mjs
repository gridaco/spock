import { createHash } from 'node:crypto';
import { readdir, readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const demoRoot = process.argv[2]
  ? path.resolve(process.argv[2])
  : fileURLToPath(new URL('../public/demo/', import.meta.url));
const manifestPath = path.join(demoRoot, 'uhura-static-bundle.json');
const manifest = JSON.parse(await readFile(manifestPath, 'utf8'));
const webBuildPath = path.join(demoRoot, 'uhura-web-build.json');
const webBuild = JSON.parse(await readFile(webBuildPath, 'utf8'));

const fail = (message) => {
  throw new Error(`Uhura www demo: ${message}`);
};
const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex');

if (manifest.protocol !== 'uhura-static-web-bundle/0') fail('unknown bundle protocol');
if (manifest.mountPath !== '/demo/') fail(`unexpected mount path ${manifest.mountPath}`);
if (manifest.playEntry !== '/demo/play') {
  fail(`unexpected Play entry ${manifest.playEntry}`);
}
if (webBuild.protocol !== 'uhura-web-build/1') fail('unknown Web build protocol');
if (webBuild.profile !== 'static-export') fail(`unexpected Web profile ${webBuild.profile}`);
if (webBuild.assetBase !== '/demo/') fail(`unexpected asset base ${webBuild.assetBase}`);
if (webBuild.mountPath !== manifest.mountPath) fail('Web build mount does not match bundle');
if (webBuild.playEntry !== manifest.playEntry) fail('Web build Play entry does not match bundle');
if (!/^[a-f0-9]{64}$/.test(manifest.sourceId)) fail('invalid application source ID');
if (!Array.isArray(manifest.files) || manifest.files.length === 0) fail('empty file manifest');
if (!Number.isInteger(manifest.previews) || manifest.previews < 1) {
  fail('bundle has no checked Editor previews');
}

const declared = new Map(manifest.files.map((file) => [file.path, file]));
if (declared.size !== manifest.files.length) fail('duplicate file path');
for (const required of [
  'api/play/provider.js',
  'api/play/stylesheet.css',
  'api/play/wasm/uhura_wasm.js',
  'api/play/wasm/uhura_wasm_bg.wasm',
]) {
  const file = declared.get(required);
  if (!file || file.bytes === 0) fail(`missing nonempty ${required}`);
}

const walk = async (directory, prefix = '') => {
  const paths = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const relative = prefix ? `${prefix}/${entry.name}` : entry.name;
    const absolute = path.join(directory, entry.name);
    if (entry.isDirectory()) paths.push(...await walk(absolute, relative));
    else if (entry.isFile()) paths.push(relative);
    else fail(`unsafe non-regular output ${relative}`);
  }
  return paths;
};

const actual = (await walk(demoRoot))
  .filter((file) => file !== 'uhura-static-bundle.json')
  .sort();
const expected = [...declared.keys()].sort();
if (JSON.stringify(actual) !== JSON.stringify(expected)) {
  fail('filesystem does not match the declared file set');
}

for (const relative of actual) {
  const bytes = await readFile(path.join(demoRoot, ...relative.split('/')));
  const entry = declared.get(relative);
  if (entry.bytes !== bytes.length) fail(`${relative} has the wrong size`);
  if (entry.sha256 !== sha256(bytes)) fail(`${relative} has the wrong digest`);
  if (typeof entry.contentType !== 'string' || entry.contentType === '') {
    fail(`${relative} has no content type`);
  }
}

const bundleId = sha256(Buffer.from(JSON.stringify(manifest.files)));
if (manifest.bundleId !== bundleId) fail('bundle ID does not match its file manifest');

console.log(
  `Uhura www demo: ${actual.length} files, bundle ${bundleId.slice(0, 12)}, source ${manifest.sourceId.slice(0, 12)}`,
);
