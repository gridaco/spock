// The /~contract IR (crates/spock-lang/src/ir.rs), typed loosely — studio is a
// pure consumer of whatever the running server publishes.

export type TypeRef =
  | { kind: "uuid" | "text" | "int" | "float" | "bool" | "timestamp" }
  | { kind: "ref"; table: string }
  | { kind: "set"; values: string[] }
  | { kind: string; table?: string; values?: string[] }

export interface DefaultVal {
  kind: "auto" | "now" | "actor" | "str" | string
  value?: unknown
}

export interface Field {
  name: string
  type: TypeRef
  optional?: boolean
  unique?: boolean
  default?: DefaultVal | null
  check?: string | null
}

export interface DerivedError {
  code: string
  kind?: string
}

export interface Table {
  name: string
  key: string[]
  fields: Field[]
  uniques?: string[][]
  anchor?: boolean
  errors: DerivedError[]
}

export interface RecordField {
  name: string
  type: TypeRef
  optional?: boolean
}

export interface RecordDef {
  name: string
  fields: RecordField[]
}

export interface Returns {
  arity: "one" | "maybe" | "many"
  of: string
  scalar?: boolean
}

export interface Param {
  name: string
  type: TypeRef
  optional?: boolean
}

export interface FnDef {
  name: string
  readonly: boolean
  params: Param[]
  returns: Returns
  errors?: string[]
  refusals?: string[]
  sql?: string[]
}

export interface Contract {
  spock: string
  module?: string
  name?: string
  tables: Table[]
  records?: RecordDef[]
  fns?: FnDef[]
  seed?: unknown
}

export interface Persona {
  actor: string
  label: string
}

export interface WhoAmI {
  actor?: unknown
  anonymous: boolean
  known: boolean
}

// route model
export type Route =
  | { kind: "overview" }
  | { kind: "tables" }
  | { kind: "fns" }
  | { kind: "records" }
  | { kind: "table"; name: string }
  | { kind: "fn"; name: string }
  | { kind: "record"; name: string }
