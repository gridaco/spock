import { api } from "@/lib/api"
import type { Table } from "@/types"

const GRAPHQL_PATH = "/graphql/v1"

export type RowInput = Readonly<Record<string, unknown>>

export interface GraphQLErrorExtensions {
  code?: string
  kind?: string
  table?: string | null
  fields?: string[]
  [key: string]: unknown
}

export interface GraphQLErrorLocation {
  line: number
  column: number
}

export interface GraphQLErrorDetail {
  message: string
  locations?: GraphQLErrorLocation[]
  path?: (string | number)[]
  extensions?: GraphQLErrorExtensions
}

export interface SingleRowMutationResult {
  __typename: string
}

/**
 * A normalized GraphQL/transport failure. Contract errors are available both
 * in `errors` and as convenient top-level properties for form-level handling.
 */
export class GraphQLMutationError extends Error {
  readonly status: number
  readonly errors: readonly GraphQLErrorDetail[]
  readonly code: string
  readonly kind?: string
  readonly table?: string | null
  readonly fields: readonly string[]

  constructor(status: number, errors: readonly GraphQLErrorDetail[], fallback: string) {
    const messages = [...new Set(errors.map((error) => error.message.trim()).filter(Boolean))]
    super(messages.join("; ") || fallback)
    this.name = "GraphQLMutationError"
    this.status = status
    this.errors = errors

    const extensions = errors[0]?.extensions
    this.code = extensions?.code ?? "graphql_error"
    this.kind = extensions?.kind
    this.table = extensions?.table
    this.fields = extensions?.fields ?? []
  }
}

/** Insert one row through the GraphQL floor, letting omitted defaults apply. */
export function insertRow(
  table: Table,
  object: RowInput,
  actor: string | null,
): Promise<SingleRowMutationResult> {
  const name = table.name
  const query = `mutation StudioInsert_${name}($object: ${name}_insert_input!) {
    result: insert_${name}_one(object: $object) {
      __typename
    }
  }`

  return executeSingleRowMutation(query, { object }, actor)
}

/** Update one row by its (possibly composite) key. Omitted `_set` fields stay unchanged. */
export function updateRow(
  table: Table,
  pkColumns: RowInput,
  changes: RowInput,
  actor: string | null,
): Promise<SingleRowMutationResult> {
  const name = table.name
  const query = `mutation StudioUpdate_${name}(
    $pkColumns: ${name}_pk_columns_input!
    $set: ${name}_set_input!
  ) {
    result: update_${name}_by_pk(pk_columns: $pkColumns, _set: $set) {
      __typename
    }
  }`

  return executeSingleRowMutation(query, { pkColumns, set: changes }, actor)
}

async function executeSingleRowMutation(
  query: string,
  variables: Readonly<Record<string, unknown>>,
  actor: string | null,
): Promise<SingleRowMutationResult> {
  const response = await api(GRAPHQL_PATH, actor, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ query, variables }),
  })

  const errors = responseErrors(response.body)
  if (errors.length > 0) {
    throw new GraphQLMutationError(response.status, errors, "GraphQL mutation failed")
  }

  if (response.status < 200 || response.status >= 300) {
    const error = transportError(response.status, response.body)
    throw new GraphQLMutationError(response.status, [error], error.message)
  }

  const result = mutationResult(response.body)
  if (!result) {
    const error: GraphQLErrorDetail = {
      message: "GraphQL mutation returned an invalid response",
      extensions: { code: "invalid_response" },
    }
    throw new GraphQLMutationError(response.status, [error], error.message)
  }

  return result
}

function responseErrors(body: unknown): GraphQLErrorDetail[] {
  if (!isRecord(body) || !Array.isArray(body.errors)) return []
  return body.errors.map(normalizeGraphQLError)
}

function normalizeGraphQLError(value: unknown): GraphQLErrorDetail {
  if (!isRecord(value)) return { message: "GraphQL mutation failed" }

  const error: GraphQLErrorDetail = {
    message: typeof value.message === "string" ? value.message : "GraphQL mutation failed",
  }
  const locations = normalizeLocations(value.locations)
  const path = normalizePath(value.path)
  const extensions = normalizeExtensions(value.extensions)
  if (locations) error.locations = locations
  if (path) error.path = path
  if (extensions) error.extensions = extensions
  return error
}

function normalizeLocations(value: unknown): GraphQLErrorLocation[] | undefined {
  if (!Array.isArray(value)) return undefined
  const locations = value.flatMap((location) => {
    if (
      !isRecord(location) ||
      typeof location.line !== "number" ||
      typeof location.column !== "number"
    ) {
      return []
    }
    return [{ line: location.line, column: location.column }]
  })
  return locations.length > 0 ? locations : undefined
}

function normalizePath(value: unknown): (string | number)[] | undefined {
  if (!Array.isArray(value)) return undefined
  const path = value.filter((part): part is string | number => {
    return typeof part === "string" || typeof part === "number"
  })
  return path.length > 0 ? path : undefined
}

function normalizeExtensions(value: unknown): GraphQLErrorExtensions | undefined {
  if (!isRecord(value)) return undefined

  const extensions: GraphQLErrorExtensions = {}
  for (const [key, item] of Object.entries(value)) {
    if (key !== "code" && key !== "kind" && key !== "table" && key !== "fields") {
      extensions[key] = item
    }
  }
  if (typeof value.code === "string") extensions.code = value.code
  if (typeof value.kind === "string") extensions.kind = value.kind
  if (typeof value.table === "string" || value.table === null) extensions.table = value.table
  if (Array.isArray(value.fields) && value.fields.every((field) => typeof field === "string")) {
    extensions.fields = value.fields
  }
  return extensions
}

function transportError(status: number, body: unknown): GraphQLErrorDetail {
  const fallback =
    status === 0 ? "Unable to reach the GraphQL endpoint" : `GraphQL request failed (HTTP ${status})`

  if (!isRecord(body) || !isRecord(body.error)) {
    return {
      message: fallback,
      extensions: { code: status === 0 ? "network" : "http_error" },
    }
  }

  return {
    message: typeof body.error.message === "string" ? body.error.message : fallback,
    extensions: {
      code:
        typeof body.error.code === "string"
          ? body.error.code
          : status === 0
            ? "network"
            : "http_error",
      ...(typeof body.error.kind === "string" ? { kind: body.error.kind } : {}),
    },
  }
}

function mutationResult(body: unknown): SingleRowMutationResult | null {
  if (!isRecord(body) || !isRecord(body.data) || !isRecord(body.data.result)) return null
  const typename = body.data.result.__typename
  return typeof typename === "string" ? { __typename: typename } : null
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null
}
