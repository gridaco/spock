# spock studio

The human-developer console (RFD 0015): a Vite + React + TypeScript SPA, styled
with Tailwind v4 + shadcn/ui (the **Mira / neutral** preset, `b1D0dv72`) and
[react-data-grid](https://github.com/adazzle/react-data-grid) for the table view.

It is a **pure consumer** of a running `spock run` server's `/~contract`: browse
schema, tables (data grid), records and functions; run fns; and — the
differentiator — **impersonate** a seed persona (the Actor selector sets
`X-Spock-Actor` on every request, so fns re-answer per persona).

## How it ships

The console is **not** served by a Node process at runtime. `pnpm build`
compiles it to static assets in `dist/` (JS/CSS + the bundled Inter font — no
CDN). `dist/` is **committed** and embedded into the `spock` binary via
`rust-embed` (`crates/spock-runtime/src/http.rs`, `StudioAssets`), served
same-origin at `/~studio`. So the end-user runs one command — `spock run
app.spock` — and the binary serves the console fully offline. Only a developer
editing the console needs Node.

## Develop

```sh
pnpm install
# in another terminal: spock run <program>.spock --port 4000
pnpm dev            # Vite dev server on :5173, proxies /~contract, /rest,
                    # /graphql, /~personas, /~whoami to :4000 (HMR)
```

## Build (regenerate the embedded bundle)

After changing anything under `src/` or `index.html`:

```sh
pnpm build          # tsc -b && vite build  ->  dist/
cargo build         # re-embeds dist/ into the binary (run from the repo root)
```

`dist/` is **gitignored** (build output doesn't belong in git). The folder is
kept via `dist/.gitkeep` so `rust-embed` compiles on a fresh checkout — but
`/~studio` is empty until you run `pnpm build`. So: **always `pnpm build` before
`cargo build`/`cargo test`** when you want a working console. (The `studio_*`
Rust tests skip themselves, with a note, when the bundle hasn't been built.)

## Conventions

- **Filenames are lower-kebab-case** for all `.ts`/`.tsx` (`table-view.tsx`,
  `err-codes.tsx`, `app-context.ts`). React component *identifiers* stay
  PascalCase (`export class TableView`).
- **Prefer classes over hooks.** The pages/views and the root orchestrator
  (`app.tsx`, everything in `views/`, `NavList`) are **class components** — they
  all follow the same shape (`static contextType = AppContext`, lifecycle
  methods, `this.state`), which is predictable and easy to manage across pages.
  Hooks are fine only when the state is **truly localized, trivial, and
  unimportant** (e.g. a throwaway toggle); avoid them for anything a page's
  behavior depends on. Pure presentational pieces with no state stay as plain
  function components (that's not a hook).

## Routing

The current view lives in the URL, so a browser reload or a shared deep link
restores it instead of dropping back to the overview. `src/lib/router.ts` maps a
`Route` ↔ a pathname under the `/~studio` base (`/~studio/table/user`,
`/~studio/fn/create_post`, `/~studio/storage`, …) using the **History API** —
`app.tsx` pushes on navigation and mirrors `popstate` (back/forward) back into
the view; no `#` hash. Because it's real paths, the server needs a matching SPA
fallback: `serve_studio_asset`/`studio_asset` in `crates/spock-runtime/src/http.rs`
serve `index.html` for any `/~studio/*` path that isn't an embedded asset (a path
whose last segment has a file extension stays a genuine 404, so a broken
`script`/`link` src still fails loudly). The impersonated **Actor** is a session
toggle, not part of the URL — a reload restores the view but resets to anonymous.

## Theme

The design system is the shadcn **Mira** style + **neutral** (monochrome) theme,
applied via `pnpm dlx shadcn@latest init --preset b1D0dv72`. To re-apply or
change it later: `pnpm dlx shadcn@latest apply --preset <code>`. react-data-grid
is bridged to the shadcn tokens in `src/index.css` (`.rdg { --rdg-*: var(--…) }`)
so the grid follows light/dark automatically.
