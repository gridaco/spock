// Pure helpers over the contract IR — the same doctrine as the old console:
// reads_actor is a heuristic body scan (RFD 0015 §8), never an authoritative bit.
import type { DefaultVal, FnDef, Returns, TypeRef } from "@/types"

export function typeStr(t: TypeRef | undefined): string {
  if (!t) return "?"
  switch (t.kind) {
    case "ref":
      return "→ " + (t.table ?? "?")
    case "set":
      return "one of {" + (t.values ?? []).join(" | ") + "}"
    default:
      return t.kind
  }
}

export function defaultStr(d: DefaultVal | null | undefined): string {
  if (!d) return ""
  switch (d.kind) {
    case "auto":
      return "= auto"
    case "now":
      return "= now"
    case "actor":
      return "= me"
    case "str":
      return "= " + JSON.stringify(d.value)
    default:
      return "= " + String(d.value)
  }
}

export function returnStr(r: Returns | undefined): string {
  if (!r) return "?"
  const base = r.of
  return r.arity === "many" ? `[${base}]` : r.arity === "maybe" ? `${base}?` : base
}

export function readsActor(fn: FnDef): boolean {
  return (fn.sql ?? []).join("\n").includes("spock_actor(")
}

export function isActorSensitive(fn: FnDef): boolean {
  return readsActor(fn) || ((fn.params ?? []).length === 0 && !fn.readonly)
}

export function fnSignature(fn: FnDef): string {
  const params = (fn.params ?? [])
    .map((p) => `${p.name}: ${typeStr(p.type)}${p.optional ? "?" : ""}`)
    .join(", ")
  return `${fn.readonly ? "" : "mut "}fn ${fn.name}(${params}) -> ${returnStr(fn.returns)}`
}

export function coerce(raw: string, type: TypeRef | undefined): unknown {
  const k = type?.kind
  if (k === "int") {
    const n = parseInt(raw, 10)
    return Number.isNaN(n) ? raw : n
  }
  if (k === "float") {
    const n = Number(raw)
    return Number.isNaN(n) ? raw : n
  }
  if (k === "bool") return raw === "true" ? true : raw === "false" ? false : raw
  return raw
}

export function cellText(v: unknown): string {
  if (v === null || v === undefined) return ""
  if (typeof v === "object") return JSON.stringify(v)
  return String(v)
}
