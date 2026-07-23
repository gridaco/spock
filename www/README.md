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
corepack pnpm@10.11.0 -C uhura/web install --frozen-lockfile
corepack pnpm@10.11.0 -C www install --frozen-lockfile
corepack pnpm@10.11.0 -C www dev
```

Validate the production output with:

```sh
bash scripts/build-www.sh
```

## Uhura demo

`/demo/` serves the checked Uhura Editor export and `/demo/play` serves its
listenerless Instagram Play session. The demo is a production website artifact;
the landing page does not own or link to it.

Rebuild the export and website with `bash scripts/build-www.sh` after updating
the pinned Uhura submodule or the Instagram example.

`www/public/demo/` is generated and ignored. Do not edit or commit it. The
Vercel Git build generates it from the exact Uhura submodule commit before
Astro builds the website. The website workflow independently exercises the
same complete source build. Both paths verify the export's declared files and
digests. Do not commit Uhura's intermediate `web/dist*`, provider, Wasm, or
Cargo build outputs.

Use `vercel dev --listen 4173` when testing Editor, Play, and application-route
history fallbacks locally. Plain `astro dev` or `astro preview` does not apply
the rewrites declared in the root `vercel.json`.

## Vercel

The Vercel project root is the repository root, not `www/`. The root
`vercel.json` installs the nested website and Uhura Web dependencies, then
bootstraps the pinned Rust toolchain and lockfile-exact `wasm-bindgen-cli`,
exports `/demo`, builds Astro, and publishes `www/dist`. Keeping that root lets
the build initialize the Uhura submodule and read the canonical documentation
and repository-owned Spock TextMate grammar without duplicating them.

Vercel's Git integration owns deployment: pull-request branches produce
Previews and `main` produces the Production deployment. GitHub Actions only
validates the source build and does not hold Vercel deployment credentials.
