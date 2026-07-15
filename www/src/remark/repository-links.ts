import { existsSync, statSync } from 'node:fs';
import { dirname, join, relative, resolve, sep } from 'node:path';
import { visit } from 'unist-util-visit';

const ROOT_ROUTES: Record<string, string> = {
  'GOVERNANCE.md': '/docs/governance/project/',
  'CONTRIBUTING.md': '/docs/contributing/',
  'CODE_OF_CONDUCT.md': '/docs/code-of-conduct/',
  'CHANGELOG.md': '/docs/changelog/',
};

export default function repositoryLinks({ repoRoot }: { repoRoot: string }) {
  const canonicalSources = new Set(
    Object.keys(ROOT_ROUTES).map((path) => join(repoRoot, path)),
  );
  const docsRoot = join(repoRoot, 'docs');

  // Remark supplies a unist tree here. Keeping the plugin structural avoids a
  // second copy of the full mdast type surface in the website package.
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  return (tree: any, file: { path?: unknown }) => {
    const source = file.path ? resolve(String(file.path)) : '';
    if (!isWithin(source, docsRoot) && !canonicalSources.has(source)) return;

    visit(tree, (node: { type?: string; url?: string }) => {
      if (!['link', 'definition'].includes(node.type ?? '') || typeof node.url !== 'string') {
        return;
      }

      if (/^(?:[a-z][a-z+.-]*:|\/|#)/i.test(node.url)) return;

      const match = node.url.match(/^([^?#]*)([?#].*)?$/);
      if (!match || !match[1]) return;

      const target = resolve(dirname(source), decodeURIComponent(match[1]));
      const suffix = match[2] ?? '';

      if (!existsSync(target)) {
        throw new Error(`${file.path}: missing link target ${node.url}`);
      }
      if (!isWithin(target, repoRoot)) {
        throw new Error(`${file.path}: link escapes repository: ${node.url}`);
      }

      const route = publishedRoute(target, repoRoot);
      if (route) {
        node.url = `${route}${suffix}`;
        return;
      }

      const sourcePath = slash(relative(repoRoot, target));
      const encoded = sourcePath.split('/').map(encodeURIComponent).join('/');
      const kind = statSync(target).isDirectory() ? 'tree' : 'blob';
      node.url = `https://github.com/gridaco/spock/${kind}/main/${encoded}${suffix}`;
    });
  };
}

function publishedRoute(target: string, repoRoot: string) {
  let sourcePath = slash(relative(repoRoot, target));

  if (statSync(target).isDirectory()) {
    if (sourcePath === 'docs/spec') return '/docs/spec/';

    const readme = join(target, 'README.md');
    if (!existsSync(readme)) return undefined;
    sourcePath = slash(relative(repoRoot, readme));
  }

  if (
    sourcePath === 'docs/rfd/TEMPLATE.md' ||
    sourcePath === 'docs/governance/meetings/0000-template.md' ||
    sourcePath.startsWith('docs/working-groups/0000-template/')
  ) {
    return undefined;
  }

  if (ROOT_ROUTES[sourcePath]) return ROOT_ROUTES[sourcePath];
  if (sourcePath === 'docs/README.md') return '/docs/';
  if (sourcePath === 'docs/rfd/0000-vision.spock') {
    return '/docs/rfd/0000-vision/';
  }
  if (!sourcePath.startsWith('docs/') || !/\.(?:md|spock)$/i.test(sourcePath)) {
    return undefined;
  }

  const route = sourcePath
    .replace(/\.(?:md|spock)$/i, '')
    .replace(/\/README$/i, '')
    .replace(/^\/+|\/+$/g, '');
  return `/${route}/`;
}

function slash(value: string) {
  return sep === '/' ? value : value.split(sep).join('/');
}

function isWithin(file: string, directory: string) {
  const path = relative(directory, file);
  return path === '' || (!path.startsWith(`..${sep}`) && path !== '..');
}
