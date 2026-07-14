// The studio table view builds PostgREST-dialect query strings that the runtime
// REST frontend (crates/spock-runtime/src/filter.rs `parse_rest`) lowers into
// the one owned predicate IR — RFD 0021. This is the studio-side mirror of that
// grammar: the operator menu, and the `filter/sort/page` → query-string builder.
// Values are always carried as `URLSearchParams` values (percent-encoded), so a
// literal `%`/`(`/`,` round-trips intact; safety on the floor is structural
// (bound `?` params), never string-escaping here.

import type { TypeRef } from "@/types"

// The v0 operator set the floor supports (filter.rs §4). `_like` (case-
// sensitive) is deliberately absent — the SQLite floor refuses it; `ilike` is
// the case-insensitive form. `isnull`/`notnull` take no value.
export type Arity = "value" | "pattern" | "list" | "unary"

export interface OpDef {
  key: string
  label: string
  // the PostgREST/SQL-ish symbol shown in the operator menu
  symbol: string
  arity: Arity
}

export const FILTER_OPS: OpDef[] = [
  { key: "eq", label: "equals", symbol: "=", arity: "value" },
  { key: "neq", label: "not equal", symbol: "<>", arity: "value" },
  { key: "gt", label: "greater than", symbol: ">", arity: "value" },
  { key: "gte", label: "greater than or equal", symbol: ">=", arity: "value" },
  { key: "lt", label: "less than", symbol: "<", arity: "value" },
  { key: "lte", label: "less than or equal", symbol: "<=", arity: "value" },
  { key: "ilike", label: "like (case-insensitive)", symbol: "~~*", arity: "pattern" },
  { key: "in", label: "in list", symbol: "in", arity: "list" },
  { key: "isnull", label: "is null", symbol: "= ∅", arity: "unary" },
  { key: "notnull", label: "is not null", symbol: "≠ ∅", arity: "unary" },
]

const OP_BY_KEY = new Map(FILTER_OPS.map((o) => [o.key, o]))

export function opDef(key: string): OpDef {
  return OP_BY_KEY.get(key) ?? FILTER_OPS[0]
}

export interface FilterRule {
  id: number
  column: string
  op: string
  value: string
}

export interface SortRule {
  column: string
  dir: "asc" | "desc"
}

// The REST value for one rule, e.g. `eq.7`, `ilike.%foo%`, `in.(a,b)`,
// `is.null`, `not.is.null`. Returns null for an incomplete rule (no column, or
// a value-taking op with an empty value) so half-typed filters simply don't
// apply; incomplete filters are never sent.
export function restValue(rule: FilterRule): string | null {
  if (!rule.column) return null
  const arity = opDef(rule.op).arity
  if (rule.op === "isnull") return "is.null"
  if (rule.op === "notnull") return "not.is.null"
  const v = rule.value.trim()
  if (v === "") return null
  if (arity === "list") {
    const inner = v.replace(/^\(/, "").replace(/\)$/, "")
    return `in.(${inner})`
  }
  return `${rule.op}.${v}`
}

// Build the `/rest/v1/{table}?…` query string from the applied filters, sorts,
// and page window. Filters use repeated keys (`?a=eq.1&a=lt.9`) — honored in
// order by the ordered parse on the floor; order/limit/offset are singletons.
export function buildQuery(
  filters: FilterRule[],
  sorts: SortRule[],
  limit: number,
  offset: number,
): string {
  const p = new URLSearchParams()
  for (const f of filters) {
    const rest = restValue(f)
    if (rest) p.append(f.column, rest)
  }
  if (sorts.length) {
    p.set("order", sorts.map((s) => `${s.column}.${s.dir}`).join(","))
  }
  p.set("limit", String(limit))
  if (offset > 0) p.set("offset", String(offset))
  return p.toString()
}

// A rule is "active" (contributes a predicate) iff it lowers to a REST value.
export function isActiveRule(rule: FilterRule): boolean {
  return restValue(rule) !== null
}

// Closed-set columns get a value dropdown;
// everything else is a free-text value.
export function setValues(type: TypeRef | undefined): string[] | null {
  return type?.kind === "set" ? (type.values ?? []) : null
}
