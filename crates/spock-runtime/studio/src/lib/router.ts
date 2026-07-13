// URL ↔ Route mapping for the studio SPA.
//
// The studio is a single-page app embedded in the spock binary and served at
// `/~studio` (crates/spock-runtime/src/http.rs). Navigation used to live purely
// in React state, so a browser reload always dropped you back on the overview.
// This module makes the current view part of the URL — via the History API, not
// a `#` hash — so a hard reload or a shared deep link restores the same view.
// The Rust side has the matching SPA fallback: any `/~studio/*` path that isn't
// an embedded asset serves the app shell, which reads window.location on boot.

import type { Route } from "@/types"

// The mount point, kept in sync with `base` in vite.config.ts. BASE_URL is
// "/~studio/"; drop the trailing slash so we can append route segments.
const BASE = import.meta.env.BASE_URL.replace(/\/+$/, "")

// A Route → absolute pathname, always rooted at BASE.
export function routeToPath(route: Route): string {
  switch (route.kind) {
    case "overview":
      return `${BASE}/`
    case "tables":
      return `${BASE}/tables`
    case "fns":
      return `${BASE}/functions`
    case "records":
      return `${BASE}/records`
    case "storage":
      return `${BASE}/storage`
    case "table":
      return `${BASE}/table/${encodeURIComponent(route.name)}`
    case "fn":
      return `${BASE}/fn/${encodeURIComponent(route.name)}`
    case "record":
      return `${BASE}/record/${encodeURIComponent(route.name)}`
  }
}

// An absolute pathname → Route. Anything unrecognized (including BASE itself, or
// a stale/hand-typed URL) falls back to the overview, so navigation never
// dead-ends on a blank view.
export function pathToRoute(pathname: string): Route {
  let rest = pathname
  if (rest.startsWith(BASE)) rest = rest.slice(BASE.length)
  rest = rest.replace(/^\/+/, "").replace(/\/+$/, "")
  if (rest === "") return { kind: "overview" }

  const slash = rest.indexOf("/")
  const head = slash === -1 ? rest : rest.slice(0, slash)
  const name = slash === -1 ? "" : decodeURIComponent(rest.slice(slash + 1))

  switch (head) {
    case "tables":
      return { kind: "tables" }
    case "functions":
      return { kind: "fns" }
    case "records":
      return { kind: "records" }
    case "storage":
      return { kind: "storage" }
    case "table":
      return name ? { kind: "table", name } : { kind: "tables" }
    case "fn":
      return name ? { kind: "fn", name } : { kind: "fns" }
    case "record":
      return name ? { kind: "record", name } : { kind: "overview" }
    default:
      return { kind: "overview" }
  }
}

// Whether two pathnames point at the same view, ignoring a trailing slash — so
// clicking the view you're already on doesn't push a duplicate history entry.
export function samePath(a: string, b: string): boolean {
  const norm = (p: string) => p.replace(/\/+$/, "")
  return norm(a) === norm(b)
}

// Document title for a route, so tabs and history entries read meaningfully.
export function routeTitle(route: Route): string {
  const suffix = "spock studio"
  switch (route.kind) {
    case "overview":
      return suffix
    case "tables":
      return `Tables · ${suffix}`
    case "fns":
      return `Functions · ${suffix}`
    case "records":
      return `Records · ${suffix}`
    case "storage":
      return `Storage · ${suffix}`
    case "table":
    case "fn":
    case "record":
      return `${route.name} · ${suffix}`
  }
}
