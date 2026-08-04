#!/usr/bin/env node

import { spawnSync } from 'node:child_process';
import {
  copyFileSync,
  existsSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = dirname(dirname(fileURLToPath(import.meta.url)));
const canonicalPath = join(repoRoot, 'llms.txt');
const publicPath = join(repoRoot, 'www', 'public', 'llms.txt');
const sourceyCli = join(
  repoRoot,
  'www',
  'node_modules',
  'sourcey',
  'dist',
  'cli.js',
);
const check = process.argv.includes('--check');
const publish = process.argv.includes('--publish');

const startHere = `## Start here

- [Project and framework overview](/docs/): What Spock is, its current scope, and its documentation map.
- [Getting started](/docs/start/): Installation, a quickstart, and the complete tutorial.
- [Spock v0 language specification](/docs/spec/v0/): The normative definition of current language behavior.
- [GraphQL dialect specification](/docs/spec/graphql/): The normative GraphQL exposure contract.
- [Design records](https://github.com/gridaco/spock/tree/main/docs/rfd): Accepted and historical design records in the source repository.
- [Governance](/docs/governance/project/): Project authority, process, and decision making.
- [Contributing](/docs/contributing/): How to report problems and contribute changes.
- [GitHub source](https://github.com/gridaco/spock): Source code, issues, and releases.`;

if (!existsSync(sourceyCli)) {
  throw new Error(
    'Sourcey is not installed. Run corepack pnpm@10.11.0 -C www install first.',
  );
}

const outputDir = mkdtempSync(join(tmpdir(), 'spock-sourcey-'));

try {
  const result = spawnSync(
    process.execPath,
    [sourceyCli, 'build', '--config', 'sourcey.config.ts', '--output', outputDir],
    { cwd: repoRoot, stdio: 'inherit' },
  );

  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);

  const generatedPath = join(outputDir, 'llms.txt');
  const sourceyOutput = readFileSync(generatedPath, 'utf8').trimEnd();
  const lines = sourceyOutput.split('\n');
  const firstSection = lines.findIndex((line) => line.startsWith('## '));

  if (firstSection === -1) {
    throw new Error('Sourcey output did not contain a documentation section');
  }

  const canonical = [
    ...lines.slice(0, firstSection),
    '',
    startHere,
    '',
    ...lines.slice(firstSection),
  ].join('\n') + '\n';
  const current = existsSync(canonicalPath)
    ? readFileSync(canonicalPath, 'utf8')
    : undefined;

  if (check && current !== canonical) {
    console.error('llms.txt is stale; run: corepack pnpm@10.11.0 -C www generate:llms');
    process.exitCode = 1;
  } else {
    if (!check && current !== canonical) {
      writeFileSync(canonicalPath, canonical, 'utf8');
      console.log('Updated llms.txt from Sourcey output.');
    } else {
      console.log('llms.txt is up to date.');
    }

    if (publish) {
      copyFileSync(canonicalPath, publicPath);
      console.log('Published the canonical file to www/public/llms.txt.');
    }
  }
} finally {
  rmSync(outputDir, { recursive: true, force: true });
}
