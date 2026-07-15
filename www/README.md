# spock.sh

The Spock website is a static [Astro](https://astro.build/) site with
[Starlight](https://starlight.astro.build/) for documentation and Tailwind CSS
for the custom landing page. Production is available at
[spock.sh](https://spock.sh).

Canonical project documentation stays in the repository root. The website
loader publishes the selected files from `../docs/` and the root governance
documents, preserving their normative, decision-record, governance, or
non-normative status. Do not copy those sources into `www/`.

## Local development

From the repository root:

```sh
corepack pnpm@10.11.0 -C www install --frozen-lockfile
corepack pnpm@10.11.0 -C www dev
```

Validate the production output with:

```sh
corepack pnpm@10.11.0 -C www build
```

## Vercel

The Vercel project root is the repository root, not `www/`. The root
`vercel.json` installs and builds the nested site and publishes `www/dist`.
Keeping that root lets the build read the canonical documentation and the
repository-owned Spock TextMate grammar without duplicating either one.
