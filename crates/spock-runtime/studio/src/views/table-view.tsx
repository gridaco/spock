import { Component, useRef, useState } from "react"
import type { ChangeEvent, ReactNode } from "react"
import { DataGrid } from "react-data-grid"
import type { Column } from "react-data-grid"
import {
  ArrowDown,
  ArrowDownUp,
  ArrowUp,
  ChevronLeft,
  ChevronRight,
  Columns3,
  Filter as FilterIcon,
  KeyRound,
  Plus,
  RefreshCw,
  Rows3,
  Upload,
  X,
} from "lucide-react"

import { api, isErrorBody } from "@/lib/api"
import { AppContext } from "@/lib/app-context"
import type { AppState } from "@/lib/app-context"
import { useApp } from "@/lib/app-context"
import { cellText, defaultStr, typeStr } from "@/lib/contract"
import { buildQuery, FILTER_OPS, isActiveRule, opDef, setValues } from "@/lib/query"
import type { FilterRule, SortRule } from "@/lib/query"
import { isFileField, setFileField, uploadFile } from "@/lib/storage"
import { cn } from "@/lib/utils"
import type { Field, Table } from "@/types"

import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card } from "@/components/ui/card"
import { Input } from "@/components/ui/input"
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { Doc } from "@/components/doc"
import { ErrCodes } from "@/components/err-codes"
import { FileThumb } from "@/components/file-thumb"

type Row = Record<string, unknown>
type Mode = "data" | "schema"

interface State {
  mode: Mode
  rows: Row[]
  limit: number
  offset: number
  filters: FilterRule[]
  sorts: SortRule[]
  loading: boolean
  err: string | null
}

// The filter/sort popovers are stateless — every mutation flows back through
// these handlers on TableView, which own the applied predicate and re-fetch.
interface FilterCtl {
  add: () => void
  remove: (id: number) => void
  setColumn: (id: number, column: string) => void
  setOp: (id: number, op: string) => void
  draftValue: (id: number, value: string) => void // update only, no re-fetch (typing)
  commitValue: (id: number, value: string) => void // apply + re-fetch (blur / enter / pick)
}

interface SortCtl {
  add: (column: string) => void
  remove: (column: string) => void
  toggleDir: (column: string) => void
}

// Filter rows need stable ids across edits; a module counter is enough (table
// switches remount via `key`, so ids never have to be reconciled).
let filterId = 0

function clampLimit(v: string): number {
  const n = parseInt(v, 10)
  if (Number.isNaN(n)) return 50
  return Math.max(1, Math.min(200, n))
}

export class TableView extends Component<{ name: string }, State> {
  static contextType = AppContext
  declare context: AppState
  state: State = {
    mode: "data",
    rows: [],
    limit: 50,
    offset: 0,
    filters: [],
    sorts: [],
    loading: false,
    err: null,
  }
  private lastActor: string | null = null
  private lastReload = -1
  private lastQuery: string | null = null

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
    const s = this.state
    if (
      prevState.mode !== s.mode ||
      prevState.rows !== s.rows ||
      prevState.limit !== s.limit ||
      prevState.offset !== s.offset ||
      prevState.filters !== s.filters ||
      prevState.sorts !== s.sorts
    ) {
      this.pushStatus()
    }
  }

  private load = async () => {
    const table = this.table()
    if (!table) return
    this.setState({ loading: true, err: null })
    const { filters, sorts, limit, offset } = this.state
    const qs = buildQuery(filters, sorts, limit, offset)
    this.lastQuery = qs
    const res = await api(`/rest/v1/${encodeURIComponent(table.name)}?${qs}`, this.context.actor)
    // a newer load() may have started (rewriting lastQuery) while this request
    // was in flight — drop the now-stale response so it can't clobber fresh rows
    if (qs !== this.lastQuery) return
    if (res.status !== 200) {
      // surface the floor's refusal (unknown_field / type_mismatch / bad_request)
      const msg = isErrorBody(res.body)
        ? `${res.body.error.code}${res.body.error.message ? " · " + res.body.error.message : ""}`
        : `HTTP ${res.status}`
      this.setState({ loading: false, err: msg, rows: [] })
      return
    }
    const body = res.body as { rows?: Row[] }
    this.setState({ loading: false, rows: body.rows ?? [] })
  }

  private setMode = (mode: Mode) => this.setState({ mode })

  // Re-fetch only when the effective query actually changes: mutating an
  // inactive filter (empty value) or re-blurring an unchanged limit rebuilds
  // the same query string, so there is nothing new to fetch. The Refresh
  // button and persona switches call `load()` directly to force a re-fetch.
  private maybeLoad = () => {
    if (!this.table()) return
    const { filters, sorts, limit, offset } = this.state
    if (buildQuery(filters, sorts, limit, offset) === this.lastQuery) return
    void this.load()
  }

  // Every predicate/sort/page edit shares one envelope: apply the change, reset
  // to the first page, then re-fetch if the query changed.
  private reloadWith = (patch: (s: State) => Partial<State>) =>
    this.setState((s) => ({ ...patch(s), offset: 0 }) as State, () => this.maybeLoad())

  // --- filter mutations ----------------------------------------------------
  private fAdd = () => {
    const f = this.table()?.fields[0]
    if (!f) return
    this.setState((s) => ({
      filters: [...s.filters, { id: ++filterId, column: f.name, op: "eq", value: "" }],
    }))
  }
  private fRemove = (id: number) =>
    this.reloadWith((s) => ({ filters: s.filters.filter((r) => r.id !== id) }))
  private mapRule = (id: number, fn: (r: FilterRule) => FilterRule, reload: boolean) => {
    const next = (s: State) => ({ filters: s.filters.map((r) => (r.id === id ? fn(r) : r)) })
    if (reload) this.reloadWith(next)
    else this.setState(next) // typing: update the draft, defer the fetch to commit
  }
  private fSetColumn = (id: number, column: string) =>
    // clear the value: it may not match the new column's type (a bool/set
    // dropdown would show a non-option, and the re-fetch would type_mismatch)
    this.mapRule(id, (r) => ({ ...r, column, value: "" }), true)
  private fSetOp = (id: number, op: string) => this.mapRule(id, (r) => ({ ...r, op }), true)
  private fDraftValue = (id: number, value: string) =>
    this.mapRule(id, (r) => ({ ...r, value }), false)
  private fCommitValue = (id: number, value: string) =>
    this.mapRule(id, (r) => ({ ...r, value }), true)

  // --- sort mutations ------------------------------------------------------
  // The picker only offers real, not-yet-sorted columns, so `sAdd` needs no guard.
  private sAdd = (column: string) =>
    this.reloadWith((s) => ({ sorts: [...s.sorts, { column, dir: "asc" }] }))
  private sRemove = (column: string) =>
    this.reloadWith((s) => ({ sorts: s.sorts.filter((x) => x.column !== column) }))
  private sToggleDir = (column: string) =>
    this.reloadWith((s) => ({
      sorts: s.sorts.map((x) =>
        x.column === column ? { ...x, dir: x.dir === "asc" ? "desc" : "asc" } : x,
      ),
    }))

  // --- paging --------------------------------------------------------------
  private page = (dir: -1 | 1) => {
    const offset = Math.max(0, this.state.offset + dir * this.state.limit)
    if (offset === this.state.offset) return
    this.setState({ offset }, () => this.maybeLoad())
  }
  private setLimit = () => this.reloadWith(() => ({}))

  private pushStatus() {
    const table = this.table()
    if (!table) return
    const { mode, rows, limit, offset, filters, sorts } = this.state
    if (mode === "schema") {
      this.context.setStatus({
        left: (
          <span>
            <b className="text-foreground">{table.fields.length}</b> fields · read-only
          </span>
        ),
      })
      return
    }
    const start = rows.length ? offset + 1 : 0
    const end = offset + rows.length
    const nFilters = filters.filter(isActiveRule).length
    this.context.setStatus({
      left: (
        <span>
          rows <b className="text-foreground">{start}</b>–<b className="text-foreground">{end}</b>
          {rows.length >= limit ? " (more)" : ""} · read-only
        </span>
      ),
      right:
        nFilters || sorts.length ? (
          <span>
            {nFilters ? `${nFilters} filter${nFilters === 1 ? "" : "s"}` : ""}
            {nFilters && sorts.length ? " · " : ""}
            {sorts.length ? `${sorts.length} sort${sorts.length === 1 ? "" : "s"}` : ""}
          </span>
        ) : undefined,
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
    const { mode, rows, limit, offset, filters, sorts, loading, err } = this.state
    const meCols = table.fields.filter((f) => f.default?.kind === "actor").map((f) => f.name)

    const filterCtl: FilterCtl = {
      add: this.fAdd,
      remove: this.fRemove,
      setColumn: this.fSetColumn,
      setOp: this.fSetOp,
      draftValue: this.fDraftValue,
      commitValue: this.fCommitValue,
    }
    const sortCtl: SortCtl = { add: this.sAdd, remove: this.sRemove, toggleDir: this.sToggleDir }

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
                <b className="text-foreground">Server-stamped:</b> {meCols.join(", ")} — set by
                the server from whoever you're acting as (<code>= me</code>); the client can't
                set or forge them.
              </Note>
            ) : null}
            <div className="flex items-center gap-2 my-3">
              <FilterButton table={table} filters={filters} ctl={filterCtl} />
              <SortButton table={table} sorts={sorts} ctl={sortCtl} />
              <div className="flex-1" />
              <Pager
                offset={offset}
                count={rows.length}
                limit={limit}
                loading={loading}
                onPage={this.page}
              />
              <label className="flex items-center gap-1.5 text-xs text-muted-foreground">
                rows
                <Input
                  type="number"
                  value={limit}
                  min={1}
                  max={200}
                  onChange={(e) => this.setState({ limit: clampLimit(e.target.value) })}
                  onBlur={this.setLimit}
                  className="w-16 h-7"
                />
              </label>
              <Button variant="outline" size="sm" onClick={() => void this.load()}>
                <RefreshCw size={14} /> Refresh
              </Button>
            </div>
            <Note>
              Table reads aren't affected by the <b className="text-foreground">Actor</b>{" "}
              selector — impersonation changes function results and <code>= me</code> stamps,
              not which rows you can read. <b className="text-foreground">Filter</b> and{" "}
              <b className="text-foreground">Sort</b> run on the server. File columns upload
              through storage; inline <b className="text-foreground">editing</b> isn't
              available yet.
            </Note>
            <div className="flex-1 min-h-0 border rounded-md overflow-hidden mt-3">
              {err ? (
                <div className="p-6 text-destructive text-sm font-mono">{err}</div>
              ) : rows.length === 0 && !loading ? (
                <div className="p-6 text-muted-foreground text-sm">
                  {filters.some(isActiveRule) ? "no rows match the filter" : "no rows"}
                </div>
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

// --- filter popover (Supabase-style) ---------------------------------------
// A funnel button whose badge counts the *applied* filters, opening a panel of
// [column][operator][value][remove] rows. Each row lowers to one PostgREST
// clause on the floor (crates/spock-runtime/src/filter.rs).
function FilterButton({
  table,
  filters,
  ctl,
}: {
  table: Table
  filters: FilterRule[]
  ctl: FilterCtl
}) {
  const active = filters.filter(isActiveRule).length
  return (
    <Popover>
      <PopoverTrigger render={<Button variant="outline" size="sm" />}>
        <FilterIcon size={14} /> Filter
        {active ? (
          <Badge variant="secondary" className="ml-0.5 h-4 min-w-4 px-1 text-[10px] tabular-nums">
            {active}
          </Badge>
        ) : null}
      </PopoverTrigger>
      <PopoverContent className="w-[420px]" align="start">
        {filters.length === 0 ? (
          <div className="px-2.5 py-3 text-[13px] text-muted-foreground">
            No filters applied to this view.
          </div>
        ) : (
          <div className="flex flex-col gap-1.5 p-1.5">
            {filters.map((r) => (
              <FilterRow key={r.id} table={table} rule={r} ctl={ctl} />
            ))}
          </div>
        )}
        <div className="border-t mt-1 pt-1">
          <button
            onClick={ctl.add}
            className="flex w-full items-center gap-1.5 rounded-md px-2.5 py-1.5 text-[13px] text-muted-foreground hover:bg-accent hover:text-foreground"
          >
            <Plus size={14} /> Add filter
          </button>
        </div>
      </PopoverContent>
    </Popover>
  )
}

function FilterRow({ table, rule, ctl }: { table: Table; rule: FilterRule; ctl: FilterCtl }) {
  const field = table.fields.find((f) => f.name === rule.column)
  return (
    <div className="flex items-center gap-1">
      <Select value={rule.column} onValueChange={(v) => ctl.setColumn(rule.id, String(v))}>
        <SelectTrigger size="sm" className="w-[118px] shrink-0">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {table.fields.map((f) => (
            <SelectItem key={f.name} value={f.name}>
              <span className="font-mono">{f.name}</span>
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
      <Select value={rule.op} onValueChange={(v) => ctl.setOp(rule.id, String(v))}>
        <SelectTrigger size="sm" className="w-[74px] shrink-0">
          <SelectValue>{() => <span className="font-mono">{opDef(rule.op).symbol}</span>}</SelectValue>
        </SelectTrigger>
        <SelectContent alignItemWithTrigger={false} className="w-56">
          {FILTER_OPS.map((o) => (
            <SelectItem key={o.key} value={o.key}>
              <span className="font-mono w-8 inline-block">{o.symbol}</span>
              <span className="text-muted-foreground">{o.label}</span>
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
      <div className="flex-1 min-w-0">
        <FilterValue field={field} rule={rule} ctl={ctl} />
      </div>
      <button
        onClick={() => ctl.remove(rule.id)}
        title="Remove filter"
        className="shrink-0 flex size-6 items-center justify-center rounded-md text-muted-foreground hover:bg-destructive/10 hover:text-destructive"
      >
        <X size={14} />
      </button>
    </div>
  )
}

// The value cell adapts to the operator and the column's type: unary ops
// (is null / is not null) take no value; closed-set and bool columns get a
// dropdown of their allowed values; everything else is free text.
function FilterValue({
  field,
  rule,
  ctl,
}: {
  field: Field | undefined
  rule: FilterRule
  ctl: FilterCtl
}) {
  const arity = opDef(rule.op).arity
  if (arity === "unary") {
    return (
      <div className="h-6 flex items-center px-2 text-[11px] text-muted-foreground italic">
        no value
      </div>
    )
  }
  // eq/neq on a closed set or a bool → a dropdown of the allowed values.
  const scalarPick = rule.op === "eq" || rule.op === "neq"
  const options = scalarPick
    ? (setValues(field?.type) ?? (field?.type.kind === "bool" ? ["true", "false"] : null))
    : null
  if (options) {
    return (
      <ValueSelect value={rule.value} options={options} onPick={(v) => ctl.commitValue(rule.id, v)} />
    )
  }
  const placeholder = arity === "pattern" ? "%text%" : arity === "list" ? "a, b, c" : "value"
  return (
    <input
      value={rule.value}
      placeholder={placeholder}
      onChange={(e) => ctl.draftValue(rule.id, e.target.value)}
      onBlur={(e) => ctl.commitValue(rule.id, e.target.value)}
      onKeyDown={(e) => {
        if (e.key === "Enter") ctl.commitValue(rule.id, (e.target as HTMLInputElement).value)
      }}
      className="h-6 w-full rounded-md border border-input bg-transparent px-2 font-mono text-[12px] outline-none focus:border-ring"
    />
  )
}

function ValueSelect({
  value,
  options,
  onPick,
}: {
  value: string
  options: string[]
  onPick: (v: string) => void
}) {
  return (
    <Select value={value} onValueChange={(v) => onPick(String(v))}>
      <SelectTrigger size="sm" className="w-full">
        <SelectValue placeholder="value" />
      </SelectTrigger>
      <SelectContent>
        {options.map((o) => (
          <SelectItem key={o} value={o}>
            <span className="font-mono">{o}</span>
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  )
}

// --- sort popover ----------------------------------------------------------
// Mirrors Supabase's Sort: applied terms as [column][asc/desc][remove] rows,
// plus a column picker to append one. The floor forces a primary-key tiebreak
// in the last term's direction, so the visible order is always a total order.
function SortButton({ table, sorts, ctl }: { table: Table; sorts: SortRule[]; ctl: SortCtl }) {
  const used = new Set(sorts.map((s) => s.column))
  const available = table.fields.filter((f) => !used.has(f.name))
  return (
    <Popover>
      <PopoverTrigger render={<Button variant="outline" size="sm" />}>
        <ArrowDownUp size={14} /> Sort
        {sorts.length ? (
          <Badge variant="secondary" className="ml-0.5 h-4 min-w-4 px-1 text-[10px] tabular-nums">
            {sorts.length}
          </Badge>
        ) : null}
      </PopoverTrigger>
      <PopoverContent className="w-[320px]" align="start">
        {sorts.length === 0 ? (
          <div className="px-2.5 py-3 text-[13px] text-muted-foreground">
            No sorts applied to this view.
          </div>
        ) : (
          <div className="flex flex-col gap-1 p-1.5">
            {sorts.map((s, i) => (
              <div key={s.column} className="flex items-center gap-1.5">
                <span className="w-4 text-[11px] text-muted-foreground tabular-nums">{i + 1}</span>
                <span className="flex-1 font-mono text-[12.5px] truncate">{s.column}</span>
                <button
                  onClick={() => ctl.toggleDir(s.column)}
                  className="flex items-center gap-1 rounded-md border px-1.5 h-6 text-[11px] hover:bg-accent"
                >
                  {s.dir === "asc" ? <ArrowUp size={12} /> : <ArrowDown size={12} />}
                  {s.dir}
                </button>
                <button
                  onClick={() => ctl.remove(s.column)}
                  title="Remove sort"
                  className="flex size-6 items-center justify-center rounded-md text-muted-foreground hover:bg-destructive/10 hover:text-destructive"
                >
                  <X size={14} />
                </button>
              </div>
            ))}
          </div>
        )}
        {available.length ? (
          <div className="border-t mt-1 pt-1">
            <div className="px-2.5 pt-1 pb-0.5 text-[11px] text-muted-foreground">
              Pick a column to sort by
            </div>
            <div className="max-h-44 overflow-y-auto">
              {available.map((f) => (
                <button
                  key={f.name}
                  onClick={() => ctl.add(f.name)}
                  className="flex w-full items-center gap-1.5 rounded-md px-2.5 py-1.5 text-[13px] text-muted-foreground hover:bg-accent hover:text-foreground"
                >
                  <Plus size={13} /> <span className="font-mono">{f.name}</span>
                </button>
              ))}
            </div>
          </div>
        ) : null}
      </PopoverContent>
    </Popover>
  )
}

// --- pager -----------------------------------------------------------------
// Offset-based prev/next over the page window. There is no total count on the
// floor (examples/filter-lab/FEEDBACK.md F7), so "next" is enabled while the
// page comes back full — the honest signal that another page may exist.
function Pager({
  offset,
  count,
  limit,
  loading,
  onPage,
}: {
  offset: number
  count: number
  limit: number
  loading: boolean
  onPage: (dir: -1 | 1) => void
}) {
  const start = count ? offset + 1 : 0
  const end = offset + count
  const canPrev = offset > 0 && !loading
  const canNext = count >= limit && count > 0 && !loading
  return (
    <div className="flex items-center gap-1 text-xs text-muted-foreground">
      <Button
        variant="outline"
        size="icon-sm"
        disabled={!canPrev}
        onClick={() => onPage(-1)}
        title="Previous page"
      >
        <ChevronLeft size={14} />
      </Button>
      <span className="tabular-nums min-w-[64px] text-center">
        {start}–{end}
      </span>
      <Button
        variant="outline"
        size="icon-sm"
        disabled={!canNext}
        onClick={() => onPage(1)}
        title="Next page"
      >
        <ChevronRight size={14} />
      </Button>
    </div>
  )
}

function SchemaMode({ table, meCols }: { table: Table; meCols: string[] }) {
  return (
    <div className="flex-1 min-h-0 overflow-y-auto px-6 pb-6 pt-4">
      {meCols.length ? (
        <Note info>
          <b className="text-foreground">Server-stamped:</b> {meCols.join(", ")} —{" "}
          <code>= me</code>, set by the server from the current actor; the client can't
          set them.
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
