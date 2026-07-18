import type { Loader, LoaderContext } from 'astro/loaders';
import matter from 'gray-matter';
import { readdir, readFile } from 'node:fs/promises';
import { join, relative, resolve, sep } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

type Authority =
  | 'normative'
  | 'governance'
  | 'decision-record'
  | 'non-normative'
  | 'informational'
  | 'guide';

// User-facing guides. Single path segment only, so a stray nested file still
// fails closed against the publication policy.
const GUIDE_PATTERN =
  /^docs\/(?:start|language|reference)\/[^/]+\.md$|^docs\/(?:status|uhura|examples)\.md$/i;

function isGuide(sourcePath: string) {
  return GUIDE_PATTERN.test(sourcePath);
}

const ROOT_FILES = [
  'GOVERNANCE.md',
  'CONTRIBUTING.md',
  'CODE_OF_CONDUCT.md',
  'CHANGELOG.md',
];

const EXCLUDED = new Set([
  'docs/governance/meetings/0000-template.md',
]);

interface SourceEntry {
  id: string;
  title: string;
  body: string;
  authority: Authority;
  banner?: string;
  description?: string;
  legacyRfd?: boolean;
  rfdDecision?: string;
  rfdImplementation?: string;
  sidebarOrder?: number;
  sidebarBadge?: unknown;
}

export function spockDocsLoader({ repoRoot }: { repoRoot: URL }): Loader {
  const repoPath = fileURLToPath(repoRoot);
  const docsPath = join(repoPath, 'docs');
  const rootPaths = ROOT_FILES.map((file) => join(repoPath, file));

  return {
    name: 'spock-docs-loader',

    async load(context) {
      const sync = () => syncDocuments(context, repoPath);
      await sync();

      if (!context.watcher) return;

      context.watcher.add([docsPath, ...rootPaths]);

      let queue = Promise.resolve();
      let debounce: ReturnType<typeof setTimeout> | undefined;

      context.watcher.on('all', (_event, changedPath) => {
        const absolute = resolve(changedPath);
        if (!isWithin(absolute, docsPath) && !rootPaths.includes(absolute)) return;

        clearTimeout(debounce);
        debounce = setTimeout(() => {
          queue = queue.then(sync).catch((error: unknown) => {
            context.logger.error(
              error instanceof Error ? (error.stack ?? error.message) : String(error),
            );
          });
        }, 75);
      });
    },
  };
}

async function syncDocuments(context: LoaderContext, repoPath: string) {
  const { config, generateDigest, parseData, renderMarkdown, store } = context;
  const docsFiles = await walk(join(repoPath, 'docs'));
  const sources: string[] = [];

  for (const sourcePath of docsFiles) {
    const sourcePathFromRoot = repoRelative(repoPath, sourcePath);
    if (isExcluded(sourcePathFromRoot)) continue;
    if (!isPublishable(sourcePathFromRoot)) {
      throw new Error(
        `Documentation source has no publication policy: ${sourcePathFromRoot}`,
      );
    }
    sources.push(sourcePath);
  }

  sources.push(...ROOT_FILES.map((file) => join(repoPath, file)));
  sources.sort();
  store.clear();

  const ids = new Set<string>();

  for (const sourcePath of sources) {
    const sourcePathFromRoot = repoRelative(repoPath, sourcePath);
    const raw = await readFile(sourcePath, 'utf8');
    const entry = sourceEntry(sourcePathFromRoot, raw);

    if (ids.has(entry.id)) {
      throw new Error(`Duplicate documentation ID: ${entry.id}`);
    }
    ids.add(entry.id);

    const data = await parseData({
      id: entry.id,
      filePath: slash(relative(fileURLToPath(config.root), sourcePath)),
      data: {
        title: entry.title,
        ...(entry.description ? { description: entry.description } : {}),
        editUrl: githubEditUrl(sourcePathFromRoot),
        lastUpdated: false,
        authority: entry.authority,
        sourcePath: sourcePathFromRoot,
        sidebar: {
          ...(entry.sidebarOrder === undefined ? {} : { order: entry.sidebarOrder }),
          ...(entry.sidebarBadge === undefined ? {} : { badge: entry.sidebarBadge }),
        },
        ...(entry.banner ? { banner: { content: entry.banner } } : {}),
        ...(entry.authority === 'non-normative'
          ? {
              pagefind: false,
              head: [
                {
                  tag: 'meta',
                  attrs: { name: 'robots', content: 'noindex,follow' },
                },
              ],
            }
          : {}),
        ...(entry.legacyRfd ? { legacyRfd: true } : {}),
        ...(entry.rfdDecision ? { rfdDecision: entry.rfdDecision } : {}),
        ...(entry.rfdImplementation
          ? { rfdImplementation: entry.rfdImplementation }
          : {}),
      },
    });

    store.set({
      id: entry.id,
      data,
      body: entry.body,
      filePath: collectionFilePath(entry.id, sourcePathFromRoot),
      rendered: await renderMarkdown(entry.body, {
        fileURL: pathToFileURL(sourcePath),
      }),
      digest: generateDigest(raw),
    });
  }

  await addSpecificationIndex(context, repoPath, ids);
  context.logger.info(`Published ${ids.size} canonical documentation pages.`);
}

async function addSpecificationIndex(
  context: LoaderContext,
  repoPath: string,
  ids: Set<string>,
) {
  const { config, generateDigest, parseData, renderMarkdown, store } = context;
  const id = 'docs/spec';

  if (ids.has(id)) throw new Error(`Duplicate documentation ID: ${id}`);
  ids.add(id);

  const body = [
    'These documents define the current behavior of the Spock language and its public dialects.',
    '',
    '- [Spock v0 language specification](/docs/spec/v0/)',
    '- [GraphQL dialect specification](/docs/spec/graphql/)',
  ].join('\n');

  const source = join(repoPath, 'docs', 'README.md');
  const filePath = slash(relative(fileURLToPath(config.root), source));
  const data = await parseData({
    id,
    filePath,
    data: {
      title: 'Specification',
      description: 'The normative specification for current Spock behavior.',
      editUrl: 'https://github.com/gridaco/spock/tree/main/docs/spec',
      lastUpdated: false,
      authority: 'normative',
      sourcePath: 'docs/spec/',
      sidebar: { order: 0 },
      banner: {
        content:
          '<strong>Normative specification.</strong> These pages define current Spock behavior for their stated scope.',
      },
    },
  });

  store.set({
    id,
    data,
    body,
    filePath: 'src/content/docs/docs/spec/index.md',
    rendered: await renderMarkdown(body, { fileURL: pathToFileURL(source) }),
    digest: generateDigest(body),
  });
}

function sourceEntry(sourcePath: string, raw: string): SourceEntry {
  const authority = authorityFor(sourcePath);

  if (sourcePath === 'docs/rfd/0000-vision.spock') {
    return {
      id: 'docs/rfd/0000-vision',
      title: 'RFD 0000 — Vision',
      body: `\`\`\`spock\n${raw.trimEnd()}\n\`\`\`\n`,
      authority,
      banner:
        '<strong>Legacy speculative source.</strong> This is not accepted Spock syntax and does not define current behavior.',
      legacyRfd: true,
      sidebarOrder: 1,
      description:
        'The original speculative Spock vision, preserved as a legacy design record.',
    };
  }

  const parsed = matter(raw);
  const heading = parsed.content.match(/^#\s+(.+?)\s*$/m);

  if (!heading || heading.index === undefined || parsed.content.slice(0, heading.index).trim()) {
    throw new Error(`${sourcePath} must begin with one level-one heading`);
  }

  const body = parsed.content
    .slice(heading.index + heading[0].length)
    .replace(/^\r?\n+/, '')
    // Expressive Code does not bundle an EBNF grammar.
    .replace(/^```ebnf\s*$/gm, '```text');

  const number = rfdNumber(sourcePath);
  if (number !== undefined && number >= 24) {
    for (const field of [
      'rfd',
      'title',
      'authors',
      'sponsor',
      'shepherd',
      'decision',
      'implementation',
    ] as const) {
      if (parsed.data[field] === undefined) {
        throw new Error(`${sourcePath} is missing required RFD frontmatter: ${field}`);
      }
    }
  }

  return {
    id: idFor(sourcePath),
    title: plainHeading(heading[1]),
    body,
    authority,
    banner: bannerFor(authority, sourcePath),
    legacyRfd: number !== undefined && number <= 23,
    description:
      typeof parsed.data.description === 'string'
        ? parsed.data.description
        : descriptionFrom(body),
    rfdDecision:
      typeof parsed.data.decision === 'string' ? parsed.data.decision : undefined,
    rfdImplementation:
      typeof parsed.data.implementation === 'string'
        ? parsed.data.implementation
        : undefined,
    sidebarOrder: sidebarOrderFor(sourcePath, number) ?? orderFrom(sourcePath, parsed.data),
    sidebarBadge: parsed.data.badge,
  };
}

function orderFrom(sourcePath: string, data: Record<string, unknown>) {
  if (data.order === undefined) return undefined;
  if (typeof data.order !== 'number') {
    throw new Error(`${sourcePath}: frontmatter "order" must be a number`);
  }
  return data.order;
}

// Starlight's autogenerated sidebar groups match entries by their file path
// relative to `src/content/docs/`. The canonical sources live outside the
// site package, so each entry is stored under a virtual collection path
// derived from its ID; directory READMEs become `index.md` so Starlight
// treats them as group index pages.
function collectionFilePath(id: string, sourcePath: string) {
  const isIndex = /\/README\.md$/i.test(sourcePath);
  return `src/content/docs/${id}${isIndex ? '/index' : ''}.md`;
}

function idFor(sourcePath: string) {
  const rootIds: Record<string, string> = {
    'GOVERNANCE.md': 'docs/governance/project',
    'CONTRIBUTING.md': 'docs/contributing',
    'CODE_OF_CONDUCT.md': 'docs/code-of-conduct',
    'CHANGELOG.md': 'docs/changelog',
  };

  if (rootIds[sourcePath]) return rootIds[sourcePath];
  if (sourcePath === 'docs/README.md') return 'docs';

  return sourcePath.replace(/\.(?:md|spock)$/i, '').replace(/\/README$/i, '');
}

function authorityFor(sourcePath: string): Authority {
  if (sourcePath.startsWith('docs/spec/')) return 'normative';
  if (/^docs\/rfd\/\d{4}-/.test(sourcePath)) return 'decision-record';

  if (
    (sourcePath.startsWith('docs/working-groups/') &&
      sourcePath !== 'docs/working-groups/README.md') ||
    (sourcePath.startsWith('docs/governance/meetings/') &&
      sourcePath !== 'docs/governance/meetings/README.md')
  ) {
    return 'non-normative';
  }

  if (
    sourcePath === 'GOVERNANCE.md' ||
    sourcePath === 'CODE_OF_CONDUCT.md' ||
    sourcePath.startsWith('docs/governance/') ||
    sourcePath === 'docs/rfd/README.md' ||
    sourcePath === 'docs/working-groups/README.md'
  ) {
    return 'governance';
  }

  if (isGuide(sourcePath)) return 'guide';

  return 'informational';
}

function bannerFor(authority: Authority, sourcePath: string) {
  switch (authority) {
    case 'normative':
      return '<strong>Normative specification.</strong> This page defines current Spock behavior for its stated scope.';
    case 'governance':
      return '<strong>Governance document.</strong> This page governs project process or conduct, not current language behavior.';
    case 'decision-record':
      return '<strong>Decision record.</strong> This page preserves design history or future direction; it does not define current Spock behavior.';
    case 'non-normative':
      return sourcePath.startsWith('docs/working-groups/')
        ? '<strong>Non-normative research.</strong> Working-group material cannot amend the Spock specification.'
        : '<strong>Meeting record.</strong> This record does not define current Spock behavior.';
    case 'guide':
      // The /docs/status/ link is an absolute URL, so remark link validation
      // does not cover it; it must track docs/status.md.
      return '<strong>Non-normative guide.</strong> This page shows how to use Spock; the <a href="/docs/spec/v0/">v0 specification</a> governs where they disagree. Spock is pre-1.0 and interfaces may change — see <a href="/docs/status/">project status</a>.';
    default:
      return undefined;
  }
}

function sidebarOrderFor(sourcePath: string, rfd?: number) {
  if (sourcePath === 'docs/README.md') return 0;
  if (/\/README\.md$/i.test(sourcePath)) return 0;
  if (rfd !== undefined) return rfd + 1;
  if (sourcePath === 'docs/spec/v0.md') return 1;
  if (sourcePath === 'docs/spec/graphql.md') return 2;
  return undefined;
}

function isPublishable(sourcePath: string) {
  if (sourcePath === 'docs/README.md') return true;
  if (isGuide(sourcePath)) return true;
  if (/^docs\/spec\/[^/]+\.md$/i.test(sourcePath)) return true;
  if (/^docs\/governance\/.+\.md$/i.test(sourcePath)) return true;
  if (sourcePath === 'docs/working-groups/README.md') return true;
  return /^docs\/working-groups\/\d{4}-[^/]+\/.+\.md$/i.test(sourcePath);
}

function isExcluded(sourcePath: string) {
  return (
    EXCLUDED.has(sourcePath) ||
    sourcePath.startsWith('docs/rfd/') ||
    sourcePath.startsWith('docs/studies/') ||
    sourcePath.startsWith('docs/working-groups/0000-template/')
  );
}

async function walk(directory: string): Promise<string[]> {
  const files: string[] = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) files.push(...(await walk(path)));
    else if (entry.isFile()) files.push(path);
  }
  return files;
}

function rfdNumber(sourcePath: string) {
  const match = sourcePath.match(/^docs\/rfd\/(\d{4})-/);
  return match ? Number(match[1]) : undefined;
}

function descriptionFrom(body: string) {
  const paragraph = body
    .replace(/```[\s\S]*?```/g, '')
    .split(/\r?\n\s*\r?\n/)
    .map((value) => value.trim())
    .find(
      (value) =>
        value.length > 30 &&
        !/^(?:#{1,6}\s|<!--|---$|\||[-*+]\s|\d+\.\s)/.test(value),
    );

  if (!paragraph) return undefined;

  const plain = paragraph
    .replace(/\[([^\]]+)\]\([^)]+\)/g, '$1')
    .replace(/[`*_~>]/g, '')
    .replace(/\s+/g, ' ')
    .trim();

  if (plain.length <= 160) return plain;
  const shortened = plain.slice(0, 157).replace(/\s+\S*$/, '');
  return `${shortened}…`;
}

function plainHeading(value: string) {
  return value.replace(/[`*_~]/g, '').replace(/\s+/g, ' ').trim();
}

function githubEditUrl(sourcePath: string) {
  const encoded = sourcePath.split('/').map(encodeURIComponent).join('/');
  return `https://github.com/gridaco/spock/edit/main/${encoded}`;
}

function repoRelative(repoPath: string, file: string) {
  return slash(relative(repoPath, file));
}

function slash(value: string) {
  return sep === '/' ? value : value.split(sep).join('/');
}

function isWithin(file: string, directory: string) {
  const path = relative(directory, file);
  return path === '' || (!path.startsWith(`..${sep}`) && path !== '..');
}
