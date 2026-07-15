// @ts-check

import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { unified } from '@astrojs/markdown-remark';
import tailwindcss from '@tailwindcss/vite';
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import repositoryLinks from './src/remark/repository-links';

const repoRoot = fileURLToPath(new URL('../', import.meta.url));

const sourceGrammar = JSON.parse(
  readFileSync(
    new URL('../editors/vscode/syntaxes/spock.tmLanguage.json', import.meta.url),
    'utf8',
  ),
);

// The repository grammar calls itself "Spock", while Markdown fences use
// lowercase `spock`. Normalize the registered Shiki language name without
// creating a second grammar source of truth.
const spockGrammar = { ...sourceGrammar, name: 'spock' };

export default defineConfig({
  site: 'https://spock.sh',
  output: 'static',
  vite: {
    plugins: [tailwindcss()],
  },
  markdown: {
    processor: unified({
      remarkPlugins: [[repositoryLinks, { repoRoot }]],
    }),
  },
  integrations: [
    starlight({
      title: 'spock',
      favicon: '/favicon.ico',
      description:
        'Build and inspect a working backend, Studio, Editor, and Uhura experience from one project.',
      customCss: ['./src/styles/custom.css'],
      components: {
        SiteTitle: './src/components/docs/site-title.astro',
      },
      expressiveCode: {
        shiki: {
          langs: [spockGrammar],
        },
      },
      head: [
        {
          tag: 'meta',
          attrs: { name: 'theme-color', content: '#f1f3f5' },
        },
        {
          tag: 'meta',
          attrs: {
            property: 'og:image',
            content: 'https://spock.sh/og.webp',
          },
        },
        {
          tag: 'meta',
          attrs: { property: 'og:image:type', content: 'image/webp' },
        },
        {
          tag: 'meta',
          attrs: { property: 'og:image:width', content: '1774' },
        },
        {
          tag: 'meta',
          attrs: { property: 'og:image:height', content: '887' },
        },
        {
          tag: 'meta',
          attrs: { property: 'og:image:alt', content: 'Spock' },
        },
        {
          tag: 'meta',
          attrs: {
            name: 'twitter:image',
            content: 'https://spock.sh/og.webp',
          },
        },
        {
          tag: 'meta',
          attrs: { name: 'twitter:image:alt', content: 'Spock' },
        },
      ],
      social: [
        {
          icon: 'github',
          label: 'GitHub',
          href: 'https://github.com/gridaco/spock',
        },
      ],
      sidebar: [
        {
          label: 'Documentation',
          items: [
            { label: 'Overview', link: '/docs/' },
            { label: 'Changelog', link: '/docs/changelog/' },
            { label: 'Contributing', link: '/docs/contributing/' },
          ],
        },
        {
          label: 'Specification',
          items: [{ autogenerate: { directory: 'docs/spec' } }],
        },
        {
          label: 'Design records',
          collapsed: true,
          items: [{ autogenerate: { directory: 'docs/rfd' } }],
        },
        {
          label: 'Governance',
          collapsed: true,
          items: [{ autogenerate: { directory: 'docs/governance' } }],
        },
        {
          label: 'Working groups',
          collapsed: true,
          items: [{ autogenerate: { directory: 'docs/working-groups' } }],
        },
      ],
    }),
  ],
});
