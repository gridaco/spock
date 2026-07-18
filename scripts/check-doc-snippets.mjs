#!/usr/bin/env node
// Verify `spock` fenced snippets in the user-facing guides with `spock check`.
//
// Fence protocol (code-fence meta after the language tag):
//   ```spock                     must pass `spock check`
//   ```spock check=fail          must fail `spock check` (any diagnostic)
//   ```spock check=fail:E022     must fail with each listed code (comma list)
//   ```spock ignore              illustrative fragment, skipped
//   ```<lang> path=<name>        written into the page's working directory
//                                before later snippets run (seed assets)
//
// `check=fail:E0NN` pins deliberately couple the docs to diagnostic codes:
// renumbering a diagnostic breaks this harness, which also keeps
// docs/reference/errors.md honest.
import { readFileSync } from 'node:fs';
import { mkdtemp, readdir, rm, stat, writeFile } from 'node:fs/promises';
import { spawnSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const TIMEOUT_MS = 30_000;
const SOURCES = [
  'docs/start',
  'docs/language',
  'docs/reference',
  'docs/status.md',
  'docs/uhura.md',
  'docs/examples.md',
];

function spockCommand() {
  if (process.env.SPOCK_BIN) return [resolve(process.env.SPOCK_BIN)];
  const cargo = readFileSync(join(repoRoot, 'Cargo.toml'), 'utf8');
  const version = cargo.match(/\[workspace\.package\][^[]*?^version = "([^"]+)"/ms)?.[1];
  if (!version) throw new Error('cannot resolve the Spock version from Cargo.toml');
  return ['npx', '--yes', `spock@${version}`];
}

function* fences(markdown) {
  const lines = markdown.split(/\r?\n/);
  for (let i = 0; i < lines.length; i++) {
    const open = lines[i].match(/^(`{3,})(\S*)[ \t]*(.*)$/);
    if (!open) continue;
    const [, ticks, lang, meta] = open;
    const start = i + 1;
    while (++i < lines.length && !lines[i].startsWith(ticks));
    yield {
      lang,
      meta: meta.trim(),
      body: lines.slice(start, i).join('\n') + '\n',
      line: start + 1,
    };
  }
}

async function pages() {
  const files = [];
  for (const source of SOURCES) {
    const path = join(repoRoot, source);
    if ((await stat(path)).isDirectory()) {
      for (const name of (await readdir(path)).sort()) {
        if (name.endsWith('.md')) files.push(join(path, name));
      }
    } else {
      files.push(path);
    }
  }
  return files;
}

const [bin, ...binArgs] = spockCommand();
const version = spawnSync(bin, [...binArgs, '--version'], { encoding: 'utf8' });
if (version.error) throw version.error;
console.log(`using ${version.stdout.trim() || bin}`);

const failures = [];
let checked = 0;
let skipped = 0;

for (const page of await pages()) {
  const relPage = page.slice(repoRoot.length + 1);
  const workdir = await mkdtemp(join(tmpdir(), 'spock-docs-'));
  let n = 0;
  try {
    for (const fence of fences(readFileSync(page, 'utf8'))) {
      const aux = fence.meta.match(/(?:^|\s)path=(\S+)/)?.[1];
      if (aux) {
        await writeFile(join(workdir, aux), fence.body);
        continue;
      }
      if (fence.lang !== 'spock') continue;
      if (/(?:^|\s)ignore(?:\s|$)/.test(fence.meta)) {
        skipped++;
        continue;
      }

      const expect = fence.meta.match(/(?:^|\s)check=fail(?::([A-Z0-9,]+))?(?:\s|$)/);
      const file = join(workdir, `snippet-${++n}.spock`);
      await writeFile(file, fence.body);
      const result = spawnSync(bin, [...binArgs, 'check', file], {
        encoding: 'utf8',
        timeout: TIMEOUT_MS,
      });
      const where = `${relPage}:${fence.line}`;
      checked++;

      if (result.error) {
        failures.push(`${where}: ${result.error.message}`);
      } else if (expect) {
        if (result.status === 0) {
          failures.push(`${where}: expected failure, but check passed`);
        } else {
          for (const code of expect[1] ? expect[1].split(',') : []) {
            if (!result.stderr.includes(`error[${code}]`)) {
              failures.push(`${where}: expected error[${code}]; got:\n${result.stderr.trim()}`);
            }
          }
        }
      } else if (result.status !== 0) {
        failures.push(`${where}: check failed:\n${result.stderr.trim()}`);
      }
    }
  } finally {
    await rm(workdir, { recursive: true, force: true });
  }
}

console.log(`checked ${checked} snippet(s), skipped ${skipped}`);
if (failures.length) {
  console.error(failures.join('\n\n'));
  process.exit(1);
}
