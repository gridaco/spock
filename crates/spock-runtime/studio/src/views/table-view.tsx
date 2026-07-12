import { Component, useRef, useState } from "react"
import type { ChangeEvent, ReactNode } from "react"
import { DataGrid } from "react-data-grid"
import type { Column } from "react-data-grid"
import { Columns3, KeyRound, RefreshCw, Rows3, Search, Upload } from "lucide-react"

import { api } from "@/lib/api"
import { AppContext } from "@/lib/app-context"
import type { AppState } from "@/lib/app-context"
import { useApp } from "@/lib/app-context"
import { cellText, defaultStr, typeStr } from "@/lib/contract"
import { isFileField, setFileField, uploadFile } from "@/lib/storage"
import { cn } from "@/lib/utils"
import type { Table } from "@/types"

import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card } from "@/components/ui/card"
import { Input } from "@/components/ui/input"
import { Doc } from "@/components/doc"
import { ErrCodes } from "@/components/err-codes"
import { FileThumb } from "@/components/file-thumb"

type Row = Record<string, unknown>
type Mode = "data" | "schema"

interface State {
  mode: Mode
  rows: Row[]
  limit: number
  loading: boolean
  err: string | null
}

function clampLimit(v: string): number {
  const n = parseInt(v, 10)
  if (Number.isNaN(n)) return 50
  return Math.max(1, Math.min(200, n))
}

export class TableView extends Component<{ name: string }, State> {
  static contextType = AppContext
  declare context: AppState
  state: State = { mode: "data", rows: [], limit: 50, loading: false, err: null }
  private lastActor: string | null = null
  private lastReload = -1

  private table(): Table | undefined {
    return this.context.contract.tables.find((t) => t.name === this.props.name)
  }

  componentDidMount() {
    this.lastActor = this.context.actor
    this.lastReload = this.context.reloadKey
    if (this.state.mode === "data") void this.load()
    this.pushStatus()
  }

  componentDidUpdate(_prev: { name: string }, prevState: State) {
    // a persona switch or refresh (from context) re-answers the open table
    const { actor, reloadKey } = this.context
    if (actor !== this.lastActor || reloadKey !== this.lastReload) {
      this.lastActor = actor
      this.lastReload = reloadKey
      if (this.state.mode === "data") void this.load()
    }
    if (
      prevState.mode !== this.state.mode ||
      prevState.rows !== this.state.rows ||
      prevState.limit !== this.state.limit
    ) {
      this.pushStatus()
    }
  }

  private load = async () => {
    const table = this.table()
    if (!table) return
    this.setState({ loading: true, err: null })
    const res = await api(
      `/rest/v1/${encodeURIComponent(table.name)}?limit=${this.state.limit}`,
      this.context.actor,
    )
    if (res.status !== 200) {
      this.setState({ loading: false, err: `HTTP ${res.status}`, rows: [] })
      return
    }
    const body = res.body as { rows?: Row[] }
    this.setState({ loading: false, rows: body.rows ?? [] })
  }

  private setMode = (mode: Mode) => this.setState({ mode })

  private pushStatus() {
    const table = this.table()
    if (!table) return
    const { mode, rows, limit } = this.state
    this.context.setStatus({
      left:
        mode === "schema" ? (
          <span>
            <b className="text-foreground">{table.fields.length}</b> fields · read-only
          </span>
        ) : (
          <span>
            <b className="text-foreground">{rows.length}</b> row{rows.length === 1 ? "" : "s"}
            {rows.length >= limit ? " (capped)" : ""} · read-only
          </span>
        ),
    })
  }

  private columns(table: Table): Column<Row>[] {
    return table.fields.map((f) => {
      const file = isFileField(f)
      return {
        key: f.name,
        name: f.name,
        resizable: true,
        width: file ? 180 : undefined,
        renderHeaderCell: () => (
          // the field's `///` doc rides along as a native hover tooltip
          <span className="flex items-center gap-1.5 normal-case" title={f.doc ?? undefined}>
            {table.key.includes(f.name) ? (
              <KeyRound size={12} className="text-muted-foreground" />
            ) : null}
            <span className="font-mono text-foreground text-[12.5px] font-semibold">{f.name}</span>
            <span className="font-mono text-muted-foreground text-[11px] font-normal">
              {typeStr(f.type)}
            </span>
          </span>
        ),
        renderCell: file
          ? ({ row }: { row: Row }) => <FileCell table={table} row={row} field={f.name} />
          : ({ row }: { row: Row }) => {
              const v = row[f.name]
              if (v === null || v === undefined)
                return <span className="text-muted-foreground italic">NULL</span>
              return <span className="font-mono">{cellText(v)}</span>
            },
      }
    })
  }

  render() {
    const table = this.table()
    if (!table) return <div className="p-6 text-muted-foreground">table not found</div>
    const { mode, rows, limit, loading, err } = this.state
    const meCols = table.fields.filter((f) => f.default?.kind === "actor").map((f) => f.name)

    return (
      <div className="h-full flex flex-col min-h-0">
        <div className="px-6 pt-5">
          <h1 className="text-xl font-semibold tracking-tight flex items-center gap-2">
            {table.name}
            {table.anchor ? <Badge variant="outline">auth anchor</Badge> : null}
          </h1>
          <p className="text-sm text-muted-foreground mt-0.5">
            table · key ({table.key.join(", ")})
            {table.anchor ? " · the identity table — its rows are the personas" : ""}
          </p>
          <Doc text={table.doc} className="text-sm mt-1.5 max-w-3xl" />
          <div className="flex gap-5 mt-4 border-b -mx-6 px-6">
            <Tab active={mode === "data"} onClick={() => this.setMode("data")} icon={<Rows3 size={14} />}>
              Data
            </Tab>
            <Tab
              active={mode === "schema"}
              onClick={() => this.setMode("schema")}
              icon={<Columns3 size={14} />}
            >
              Columns
            </Tab>
          </div>
        </div>

        {mode === "data" ? (
          <div className="flex flex-col min-h-0 flex-1 px-6 pb-6 pt-4">
            {meCols.length ? (
              <Note info>
                <b className="text-foreground">Server-stamped:</b> {meCols.join(", ")} — populated
                from the current actor (<code>= me</code>), unforgeable on the floor. Provenance, not
                governance.
              </Note>
            ) : null}
            <div className="flex items-center gap-2.5 my-3">
              <div className="flex items-center gap-2 border rounded-md px-3 py-1.5 bg-muted/40 text-muted-foreground text-[12.5px] min-w-[268px]">
                <Search size={14} /> Filter — waits on the filter RFD
              </div>
              <div className="flex-1" />
              <label className="flex items-center gap-2 text-xs text-muted-foreground">
                rows
                <Input
                  type="number"
                  value={limit}
                  min={1}
                  max={200}
                  onChange={(e) => this.setState({ limit: clampLimit(e.target.value) })}
                  className="w-20 h-8"
                />
              </label>
              <Button size="sm" onClick={() => void this.load()}>
                <RefreshCw size={14} /> Load
              </Button>
            </div>
            <Note>
              Reads are <b className="text-foreground">actor-blind</b> in v0 — impersonation changes{" "}
              <b className="text-foreground">fn</b> results and <code>= me</code> stamps, not table
              reads. <b className="text-foreground">File</b> columns (→ <code>storage_object</code>)
              upload through the storage gate and attach via the GraphQL floor; other inline{" "}
              <b className="text-foreground">editing</b> waits on REST writes, and{" "}
              <b className="text-foreground">filtering</b> waits on the filter RFD.
            </Note>
            <div className="flex-1 min-h-0 border rounded-md overflow-hidden mt-3">
              {err ? (
                <div className="p-6 text-destructive text-sm">{err}</div>
              ) : rows.length === 0 && !loading ? (
                <div className="p-6 text-muted-foreground text-sm">no rows</div>
              ) : (
                <DataGrid
                  columns={this.columns(table)}
                  rows={rows}
                  rowHeight={34}
                  headerRowHeight={36}
                  style={{ blockSize: "100%" }}
                />
              )}
            </div>
          </div>
        ) : (
          <SchemaMode table={table} meCols={meCols} />
        )}
      </div>
    )
  }
}

// The Data / Columns switch, surfaced as a tab strip in the table header so
// the schema view is reachable in-flow (it used to hide in the status bar).
function Tab({
  active,
  onClick,
  icon,
  children,
}: {
  active: boolean
  onClick: () => void
  icon: ReactNode
  children: ReactNode
}) {
  return (
    <button
      onClick={onClick}
      className={cn(
        "-mb-px flex items-center gap-1.5 border-b-2 px-0.5 pb-2 text-[13px] transition-colors",
        active
          ? "border-foreground text-foreground font-medium [&_svg]:text-foreground"
          : "border-transparent text-muted-foreground hover:text-foreground [&_svg]:text-muted-foreground",
      )}
    >
      {icon}
      {children}
    </button>
  )
}

function SchemaMode({ table, meCols }: { table: Table; meCols: string[] }) {
  return (
    <div className="flex-1 min-h-0 overflow-y-auto px-6 pb-6 pt-4">
      {meCols.length ? (
        <Note info>
          <b className="text-foreground">Server-stamped:</b> {meCols.join(", ")} — <code>= me</code>,
          unforgeable on the floor. Provenance, not governance.
        </Note>
      ) : null}
      <Card className="overflow-hidden p-0 mt-3">
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b text-xs uppercase tracking-wide text-muted-foreground">
              <th className="text-left font-medium px-3 py-2 w-10">#</th>
              <th className="text-left font-medium px-3 py-2">Field</th>
              <th className="text-left font-medium px-3 py-2">Type</th>
              <th className="text-left font-medium px-3 py-2">Attributes</th>
            </tr>
          </thead>
          <tbody>
            {table.fields.map((f, i) => {
              const isKey = table.key.includes(f.name)
              return (
                <tr key={f.name} className="border-b last:border-0">
                  <td className="px-3 py-2 text-muted-foreground font-mono">{i + 1}</td>
                  <td className="px-3 py-2 font-mono align-top">
                    <span className="flex items-center gap-1.5">
                      {isKey ? <KeyRound size={12} className="text-muted-foreground" /> : null}
                      {f.name}
                    </span>
                    <Doc text={f.doc} className="text-xs mt-0.5 font-normal max-w-md" />
                  </td>
                  <td className="px-3 py-2 font-mono">
                    {typeStr(f.type)}{" "}
                    {f.default && f.default.kind !== "actor" ? (
                      <span className="text-muted-foreground">{defaultStr(f.default)}</span>
                    ) : null}
                  </td>
                  <td className="px-3 py-2">
                    <span className="flex flex-wrap gap-1">
                      {isKey ? <Badge variant="outline">key</Badge> : null}
                      {f.unique && !isKey ? <Badge variant="outline">unique</Badge> : null}
                      {f.optional ? <Badge variant="outline">optional</Badge> : null}
                      {f.default?.kind === "actor" ? <Badge variant="outline">= me</Badge> : null}
                      {f.check ? <Badge variant="outline">check {f.check}</Badge> : null}
                    </span>
                  </td>
                </tr>
              )
            })}
          </tbody>
        </table>
      </Card>
      {table.errors.length ? (
        <>
          <h2 className="text-xs uppercase tracking-wide text-muted-foreground font-semibold mt-6 mb-2">
            Derived errors
          </h2>
          <ErrCodes codes={table.errors.map((e) => ({ code: e.code, kind: e.kind }))} />
        </>
      ) : null}
    </div>
  )
}

// An in-grid file column (RFD 0018): a thumbnail of the attached object plus an
// upload/replace control. Picking a file runs the storage gate (mint → PUT)
// then attaches the new object id to this row through the GraphQL floor, all
// under the current persona, and refreshes.
function FileCell({ table, row, field }: { table: Table; row: Row; field: string }) {
  const app = useApp()
  const inputRef = useRef<HTMLInputElement>(null)
  const [busy, setBusy] = useState(false)
  const [err, setErr] = useState<string | null>(null)
  const value = (row[field] ?? null) as string | null

  const onPick = async (e: ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0]
    if (inputRef.current) inputRef.current.value = ""
    if (!file) return
    setBusy(true)
    setErr(null)
    try {
      const id = await uploadFile(file, app.actor)
      const keyValues = Object.fromEntries(table.key.map((k) => [k, row[k]]))
      await setFileField(table, keyValues, field, id, app.actor)
      app.reload()
    } catch (ex) {
      setErr(ex instanceof Error ? ex.message : String(ex))
    } finally {
      setBusy(false)
    }
  }

  return (
    <div className="flex items-center gap-1.5 h-full">
      <input ref={inputRef} type="file" hidden onChange={onPick} />
      {value ? (
        <FileThumb id={value} size={22} />
      ) : (
        <span className="text-muted-foreground italic text-xs">NULL</span>
      )}
      <button
        onClick={() => inputRef.current?.click()}
        disabled={busy}
        title={value ? "Replace file" : "Upload a file"}
        className="inline-flex items-center gap-1 rounded border px-1.5 py-0.5 text-[11px] text-muted-foreground hover:text-foreground hover:bg-accent disabled:opacity-50"
      >
        <Upload size={11} /> {busy ? "…" : value ? "Replace" : "Upload"}
      </button>
      {err ? (
        <span className="text-destructive text-xs" title={err}>
          !
        </span>
      ) : null}
    </div>
  )
}

function Note({ children, info }: { children: ReactNode; info?: boolean }) {
  return (
    <div
      className={cn(
        "border-l-2 rounded-r-md px-3.5 py-2.5 text-[13px] text-muted-foreground my-1.5 bg-muted/40",
        info ? "border-primary/50" : "border-muted-foreground/40",
      )}
    >
      {children}
    </div>
  )
}
