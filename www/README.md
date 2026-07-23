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

## Uhura demo

`/demo/` serves the checked Uhura Editor export and `/demo/play` serves its
listenerless Instagram Play session. The demo is a production website artifact;
the landing page does not own or link to it.

Rebuild the export from the repository root after updating the pinned Uhura
submodule or the Instagram example:

```sh
corepack pnpm@10.11.0 -C uhura/web install --frozen-lockfile
bash scripts/build-www-demo.sh
corepack pnpm@10.11.0 -C www run check:demo
```

`www/public/demo/` is generated and ignored. Do not edit or commit it. The
website workflow builds it from the exact Uhura submodule commit, verifies its
declared files and digests, and hands those bytes to the production deployment
job as a short-lived artifact. The cache is only an optimization; restored
bundles are verified exactly like fresh builds.

Every website build requires a valid demo. A production deployment is first
staged without assigning the production domain, exercised in a real browser,
and only then promoted without rebuilding. Do not commit Uhura's intermediate
`web/dist*`, provider, Wasm, or Cargo build outputs.

Use `vercel dev --listen 4173` when testing Editor, Play, and application-route
history fallbacks locally. Plain `astro dev` or `astro preview` does not apply
the rewrites declared in the root `vercel.json`.

## Vercel

The Vercel project root is the repository root, not `www/`. The root
`vercel.json` installs and builds the nested site and publishes `www/dist`.
Keeping that root lets the build read the canonical documentation and the
repository-owned Spock TextMate grammar without duplicating either one.

Production deployment is owned by the `www` GitHub Actions workflow, not
Vercel's Git integration. Its `Production` GitHub environment requires a
team-scoped `VERCEL_TOKEN` and the project's
`VERCEL_AUTOMATION_BYPASS_SECRET`. The latter must match Vercel's Deployment
Protection bypass-for-automation secret so the workflow can browser-test the
unaliased deployment before promotion.
