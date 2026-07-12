// The storage gate (RFD 0018), from the console's side. Studio drives the same
// Supabase-shaped `/storage/v1` endpoints any client would: mint a signed
// upload URL, PUT the bytes, then (for the in-row case) attach the object id to
// a row through the GraphQL floor. Every call carries the impersonation header,
// so `owner = me` is stamped from the selected persona.

import { api, isErrorBody } from "@/lib/api"
import type { Contract, Field, Table } from "@/types"

/** The protocol-owned metadata table name (spock_lang::ir::STORAGE_OBJECT_TABLE). */
export const STORAGE_OBJECT = "storage_object"

/** Does this contract expose the storage surface? */
export function hasStorage(contract: Contract): boolean {
  return storageTable(contract) !== undefined
}

export function storageTable(contract: Contract): Table | undefined {
  return contract.tables.find((t) => t.name === STORAGE_OBJECT)
}

/** A field that references a stored file (a `storage_object`). */
export function isFileField(field: Field): boolean {
  return field.type.kind === "ref" && (field.type as { table?: string }).table === STORAGE_OBJECT
}

/** Mint a pending object and upload the bytes. Returns the new object id. */
export async function uploadFile(file: File, actor: string | null): Promise<string> {
  const mint = await api("/storage/v1/object/upload/sign", actor, { method: "POST" })
  if (mint.status !== 200) throw new Error(envelopeError(mint, "mint"))
  const { id, url } = mint.body as { id: string; url: string }
  const put = await api(url, actor, {
    method: "PUT",
    headers: { "Content-Type": file.type || "application/octet-stream" },
    body: file,
  })
  if (put.status !== 204 && put.status !== 200) throw new Error(envelopeError(put, "upload"))
  return id
}

/** A short-lived signed GET url for a committed object, or null (e.g. pending). */
export async function signDownload(id: string, actor: string | null): Promise<string | null> {
  const r = await api(`/storage/v1/object/sign/${encodeURIComponent(id)}`, actor, {
    method: "POST",
  })
  if (r.status !== 200) return null
  return (r.body as { url?: string }).url ?? null
}

/** Attach (or clear) a storage_object reference on a row via the GraphQL floor. */
export async function setFileField(
  table: Table,
  keyValues: Record<string, unknown>,
  field: string,
  objectId: string | null,
  actor: string | null,
): Promise<void> {
  const pk = table.key.map((k) => `${k}: ${JSON.stringify(keyValues[k] ?? null)}`).join(", ")
  const val = objectId === null ? "null" : JSON.stringify(objectId)
  const query = `mutation { update_${table.name}_by_pk(pk_columns: {${pk}}, _set: {${field}: ${val}}) { ${table.key[0]} } }`
  const r = await api("/graphql/v1", actor, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ query }),
  })
  const body = (r.body ?? {}) as { errors?: { message?: string }[] }
  if (body.errors?.length) throw new Error(body.errors[0].message ?? "attach failed")
  if (r.status !== 200) throw new Error(`attach failed (HTTP ${r.status})`)
}

function envelopeError(res: { status: number; body: unknown }, what: string): string {
  const fallback = `${what} failed (HTTP ${res.status})`
  return isErrorBody(res.body) ? (res.body.error.message ?? fallback) : fallback
}
