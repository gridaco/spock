import { Component } from "react"
import type { ReactNode } from "react"
import {
  Braces,
  Check,
  Code2,
  Database,
  ExternalLink,
  HardDrive,
  LayoutDashboard,
  Moon,
  RefreshCw,
  Search,
  Sun,
  Table2,
  User,
} from "lucide-react"

import { api } from "@/lib/api"
import {
  actorFromSelectValue,
  actorSelectValue,
  ANONYMOUS_ACTOR_SELECT_VALUE,
} from "@/lib/actor"
import { AppContext } from "@/lib/app-context"
import type { AppState, StatusContent } from "@/lib/app-context"
import { pathToRoute, routeTitle, routeToPath, samePath } from "@/lib/router"
import { isDark, toggleTheme } from "@/lib/theme"
import { isActorSensitive } from "@/lib/contract"
import { hasStorage, userTables } from "@/lib/storage"
import { cn } from "@/lib/utils"
import type { Contract, Persona, Route, WhoAmI } from "@/types"

import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"

import { FnRunner } from "@/views/fn-runner"
import { FunctionsOverview } from "@/views/functions-overview"
import { Overview } from "@/views/overview"
import { RecordView } from "@/views/record-view"
import { StorageView } from "@/views/storage-view"
import { TableView } from "@/views/table-view"
import { TablesOverview } from "@/views/tables-overview"

// Prefer classes over hooks for the pages/orchestrator (studio/README.md): the
// root owns all shared state and exposes it through AppContext. Views are
// classes too; only trivial, localized, throwaway state uses hooks.
interface AppData {
  contract: Contract | null
  personas: Persona[]
  actor: string | null
  route: Route
  reloadKey: number
  status: StatusContent
  whoami: WhoAmI | null
  dark: boolean
  bootError: string | null
}

export default class App extends Component<Record<string, never>, AppData> {
  state: AppData = {
    contract: null,
    personas: [],
    actor: null,
    // The view lives in the URL (lib/router.ts), so a reload or deep link
    // restores it instead of resetting to the overview.
    route: pathToRoute(window.location.pathname),
    reloadKey: 0,
    status: {},
    whoami: null,
    dark: isDark(),
    bootError: null,
  }

  async componentDidMount() {
    // Back/forward navigation drives the view straight off the URL.
    window.addEventListener("popstate", this.onPopState)
    document.title = routeTitle(this.state.route)
    const c = await api("/~contract", null)
    if (c.status !== 200) {
      this.setState({ bootError: `could not load /~contract (HTTP ${c.status})` })
      return
    }
    this.setState({ contract: c.body as Contract })
    await this.refreshPersonas()
    void this.refreshWhoami()
  }

  componentWillUnmount() {
    window.removeEventListener("popstate", this.onPopState)
  }

  componentDidUpdate(_prevProps: Record<string, never>, prev: AppData) {
    if (prev.actor !== this.state.actor || prev.reloadKey !== this.state.reloadKey) {
      void this.refreshWhoami()
    }
  }

  private refreshWhoami = async () => {
    const r = await api("/~whoami", this.state.actor)
    this.setState({ whoami: r.body as WhoAmI })
  }

  private refreshPersonas = async () => {
    const response = await api("/~personas", null)
    if (response.status === 200) {
      this.setState({ personas: normalizePersonas(response.body) })
    }
  }

  // A user clicked something: push the target onto history (skipping a no-op
  // that would just duplicate the current entry) and render it.
  private navigate = (route: Route) => {
    const path = routeToPath(route)
    if (!samePath(path, window.location.pathname)) {
      window.history.pushState(null, "", path)
    }
    document.title = routeTitle(route)
    this.setState({ route, status: {} })
  }

  // The URL changed under us (back/forward button): mirror it into the view
  // without pushing a new entry.
  private onPopState = () => {
    const route = pathToRoute(window.location.pathname)
    document.title = routeTitle(route)
    this.setState({ route, status: {} })
  }
  private reload = () => this.setState((s) => ({ reloadKey: s.reloadKey + 1 }))
  private setActor = (actor: string | null) => this.setState({ actor })
  private setStatus = (status: StatusContent) => this.setState({ status })
  private onToggleTheme = () => this.setState({ dark: toggleTheme() })

  render() {
    const { contract, personas, actor, route, reloadKey, status, whoami, dark, bootError } =
      this.state
    if (bootError) return <div className="p-10 text-muted-foreground">{bootError}</div>
    if (!contract) return <div className="p-10 text-muted-foreground">loading…</div>

    const ctx: AppState = {
      contract,
      personas,
      refreshPersonas: this.refreshPersonas,
      actor,
      setActor: this.setActor,
      route,
      navigate: this.navigate,
      reloadKey,
      reload: this.reload,
      setStatus: this.setStatus,
    }

    return (
      <AppContext.Provider value={ctx}>
        <div className="grid h-screen" style={{ gridTemplateColumns: "216px 264px minmax(0,1fr)" }}>
          <Rail
            contract={contract}
            route={route}
            navigate={this.navigate}
            dark={dark}
            onToggleTheme={this.onToggleTheme}
          />
          <NavList contract={contract} route={route} navigate={this.navigate} />
          <div
            className="grid min-w-0 min-h-0"
            style={{ gridTemplateRows: "48px minmax(0,1fr) 34px" }}
          >
            <Topbar
              personas={personas}
              actor={actor}
              setActor={this.setActor}
              whoami={whoami}
              route={route}
              onRefresh={this.reload}
            />
            <main className="overflow-hidden min-w-0 min-h-0">{renderView(route)}</main>
            <StatusBar status={status} />
          </div>
        </div>
      </AppContext.Provider>
    )
  }
}

function renderView(route: Route): ReactNode {
  switch (route.kind) {
    case "overview":
    // Records have per-record pages but no dedicated overview view, so any
    // records-level path (`/~studio/records`, or a nameless `/~studio/record/`)
    // falls back to the home summary rather than rendering blank.
    case "records":
      return <Overview />
    case "tables":
      return <TablesOverview />
    case "fns":
      return <FunctionsOverview />
    case "table":
      return <TableView key={route.name} name={route.name} />
    case "fn":
      return <FnRunner key={route.name} name={route.name} />
    case "record":
      return <RecordView key={route.name} name={route.name} />
    case "storage":
      return <StorageView />
    default:
      return null
  }
}

function sectionOf(route: Route): string {
  switch (route.kind) {
    case "overview":
      return "overview"
    case "tables":
    case "table":
      return "tables"
    case "fns":
    case "fn":
      return "fns"
    case "storage":
      return "storage"
    default:
      return ""
  }
}

// --- rail ------------------------------------------------------------------
function Rail({
  contract,
  route,
  navigate,
  dark,
  onToggleTheme,
}: {
  contract: Contract
  route: Route
  navigate: (r: Route) => void
  dark: boolean
  onToggleTheme: () => void
}) {
  const sec = sectionOf(route)
  return (
    <aside className="flex flex-col min-h-0 bg-sidebar border-r">
      <div className="flex items-center gap-2.5 px-4 pt-4 pb-3">
        <span className="text-lg leading-none">🖖</span>
        <div className="flex flex-col leading-tight">
          <span className="text-sm font-semibold">spock</span>
          <span className="text-[11px] text-muted-foreground">studio</span>
        </div>
      </div>
      <nav className="flex flex-col gap-0.5 px-2">
        <RailItem
          icon={<LayoutDashboard size={16} />}
          label="Overview"
          active={sec === "overview"}
          onClick={() => navigate({ kind: "overview" })}
        />
        <RailItem
          icon={<Table2 size={16} />}
          label="Tables"
          active={sec === "tables"}
          onClick={() => navigate({ kind: "tables" })}
        />
        {contract.fns?.length ? (
          <RailItem
            icon={<Code2 size={16} />}
            label="Functions"
            active={sec === "fns"}
            onClick={() => navigate({ kind: "fns" })}
          />
        ) : null}
        {hasStorage(contract) ? (
          <RailItem
            icon={<HardDrive size={16} />}
            label="Storage"
            active={sec === "storage"}
            onClick={() => navigate({ kind: "storage" })}
          />
        ) : null}
      </nav>
      <div className="flex-1" />
      <div className="p-2 border-t flex flex-col gap-0.5">
        <a
          href="/graphql/v1"
          target="_blank"
          rel="noopener"
          className="flex items-center gap-2.5 rounded-md px-2.5 py-1.5 text-[13px] text-muted-foreground hover:bg-accent hover:text-foreground"
        >
          <ExternalLink size={16} /> GraphiQL
        </a>
        <button
          onClick={onToggleTheme}
          className="flex items-center gap-2.5 rounded-md px-2.5 py-1.5 text-[13px] text-muted-foreground hover:bg-accent hover:text-foreground"
        >
          {dark ? <Sun size={16} /> : <Moon size={16} />} Theme
        </button>
        <div className="px-2.5 pt-1.5 text-[11px] font-mono text-muted-foreground">/~studio</div>
      </div>
    </aside>
  )
}

function RailItem({
  icon,
  label,
  active,
  onClick,
}: {
  icon: ReactNode
  label: string
  active: boolean
  onClick: () => void
}) {
  return (
    <button
      onClick={onClick}
      className={cn(
        "flex items-center gap-2.5 w-full text-left rounded-md px-2.5 py-1.5 text-[13px]",
        active
          ? "bg-accent text-foreground font-medium [&_svg]:text-foreground"
          : "text-muted-foreground hover:bg-accent/60 hover:text-foreground",
      )}
    >
      {icon}
      {label}
    </button>
  )
}

// --- object list -----------------------------------------------------------
// A class: the search box holds trivial, localized state, but keeping the whole
// list as a class matches the "prefer classes for pages" convention.
class NavList extends Component<
  { contract: Contract; route: Route; navigate: (r: Route) => void },
  { q: string }
> {
  state = { q: "" }

  render() {
    const { contract, route, navigate } = this.props
    const q = this.state.q
    const mod = contract.module || contract.name || "contract"
    const match = (name: string) => !q || name.toLowerCase().includes(q.toLowerCase())
    return (
      <nav className="flex flex-col min-h-0 border-r bg-background">
        <div className="p-3 border-b">
          <div className="flex items-center gap-2 text-xs text-muted-foreground border rounded-md px-2.5 py-1.5 bg-muted/40 mb-2">
            <Database size={14} /> schema <b className="text-foreground font-medium">{mod}</b>
          </div>
          <div className="flex items-center gap-2 border rounded-md px-2.5 py-1.5">
            <Search size={14} className="text-muted-foreground" />
            <input
              value={q}
              onChange={(e) => this.setState({ q: e.target.value })}
              placeholder="Search…"
              className="w-full bg-transparent text-[13px] outline-none"
            />
          </div>
        </div>
        <div className="overflow-y-auto px-2 pb-10 pt-1">
          <NavGroup label="Overview" />
          <NavItem
            icon={<LayoutDashboard size={15} />}
            name="Overview"
            active={route.kind === "overview"}
            onClick={() => navigate({ kind: "overview" })}
          />
          {hasStorage(contract) ? (
            <NavItem
              icon={<HardDrive size={15} />}
              name="Storage"
              active={route.kind === "storage"}
              onClick={() => navigate({ kind: "storage" })}
            />
          ) : null}
          {userTables(contract).length ? <NavGroup label="Tables" /> : null}
          {userTables(contract)
            .filter((t) => match(t.name))
            .map((t) => (
              <NavItem
                key={t.name}
                icon={<Table2 size={15} />}
                name={t.name}
                active={route.kind === "table" && route.name === t.name}
                badge={
                  t.anchor ? (
                    <Badge variant="outline" className="text-[10px] px-1.5">
                      auth
                    </Badge>
                  ) : undefined
                }
                onClick={() => navigate({ kind: "table", name: t.name })}
              />
            ))}
          {contract.records?.length ? <NavGroup label="Records" /> : null}
          {contract.records
            ?.filter((r) => match(r.name))
            .map((r) => (
              <NavItem
                key={r.name}
                icon={<Braces size={15} />}
                name={r.name}
                active={route.kind === "record" && route.name === r.name}
                onClick={() => navigate({ kind: "record", name: r.name })}
              />
            ))}
          {contract.fns?.length ? <NavGroup label="Functions" /> : null}
          {contract.fns
            ?.filter((f) => match(f.name))
            .map((f) => (
              <NavItem
                key={f.name}
                icon={<Code2 size={15} />}
                name={f.name}
                active={route.kind === "fn" && route.name === f.name}
                badge={
                  <span className="flex items-center gap-1">
                    <Badge variant="outline" className="text-[10px] px-1.5">
                      {f.readonly ? "read" : "mut"}
                    </Badge>
                    {isActorSensitive(f) ? (
                      <Badge variant="outline" className="text-[10px] px-1.5">
                        me?
                      </Badge>
                    ) : null}
                  </span>
                }
                onClick={() => navigate({ kind: "fn", name: f.name })}
              />
            ))}
        </div>
      </nav>
    )
  }
}

function NavGroup({ label }: { label: string }) {
  return (
    <div className="px-2 pt-3 pb-1 text-[10.5px] font-medium uppercase tracking-wide text-muted-foreground">
      {label}
    </div>
  )
}

function NavItem({
  icon,
  name,
  active,
  badge,
  onClick,
}: {
  icon: ReactNode
  name: string
  active: boolean
  badge?: ReactNode
  onClick: () => void
}) {
  return (
    <button
      onClick={onClick}
      className={cn(
        "flex items-center gap-2.5 w-full text-left rounded-md px-2.5 py-1.5",
        active
          ? "bg-accent text-foreground [&_svg]:text-foreground"
          : "text-muted-foreground hover:bg-accent/60 hover:text-foreground",
      )}
    >
      {icon}
      <span className="font-mono text-[12.5px] truncate">{name}</span>
      {badge ? <span className="ml-auto flex items-center gap-1">{badge}</span> : null}
    </button>
  )
}

// --- topbar ----------------------------------------------------------------
function tabLabel(route: Route): string {
  switch (route.kind) {
    case "overview":
      return "Overview"
    case "tables":
      return "Tables"
    case "fns":
      return "Functions"
    case "records":
      return "Records"
    case "storage":
      return "Storage"
    default:
      return route.name
  }
}

function tabIcon(route: Route): ReactNode {
  switch (route.kind) {
    case "overview":
      return <LayoutDashboard size={15} />
    case "tables":
    case "table":
      return <Table2 size={15} />
    case "fns":
    case "fn":
      return <Code2 size={15} />
    case "records":
    case "record":
      return <Braces size={15} />
    case "storage":
      return <HardDrive size={15} />
    default:
      return null
  }
}

function Topbar({
  personas,
  actor,
  setActor,
  whoami,
  route,
  onRefresh,
}: {
  personas: Persona[]
  actor: string | null
  setActor: (a: string | null) => void
  whoami: WhoAmI | null
  route: Route
  onRefresh: () => void
}) {
  return (
    <header className="flex items-center gap-2.5 px-3.5 border-b bg-background">
      <div className="flex items-center gap-2 px-3 py-1 rounded-md bg-accent text-sm font-medium">
        {tabIcon(route)}
        <span>{tabLabel(route)}</span>
      </div>
      <div className="flex-1" />
      <div
        className="flex items-center gap-2 border rounded-md pl-2.5 pr-1 h-8 shadow-xs"
        title="Impersonate a seed persona — every request carries X-Spock-Actor"
      >
        <User size={14} className="text-muted-foreground" />
        <span className="text-xs text-muted-foreground">Actor</span>
        <Select
          value={actorSelectValue(actor)}
          onValueChange={(value) => setActor(actorFromSelectValue(value))}
        >
          <SelectTrigger
            size="sm"
            className="border-0 bg-transparent dark:bg-transparent shadow-none font-medium focus-visible:ring-0"
          >
            <SelectValue>
              {(value: unknown) => {
                const selectedActor = actorFromSelectValue(value)
                return selectedActor === null
                  ? "anonymous"
                  : (personas.find((persona) => persona.actor === selectedActor)?.label ??
                      selectedActor)
              }}
            </SelectValue>
          </SelectTrigger>
          <SelectContent>
            <SelectItem value={ANONYMOUS_ACTOR_SELECT_VALUE}>anonymous</SelectItem>
            {personas.map((p) => (
              <SelectItem key={p.actor} value={actorSelectValue(p.actor)}>
                {p.label}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>
      <WhoAmIBadge whoami={whoami} personas={personas} />
      <Button variant="outline" size="icon" className="size-8" onClick={onRefresh} title="Re-run">
        <RefreshCw size={15} />
      </Button>
    </header>
  )
}

function WhoAmIBadge({ whoami, personas }: { whoami: WhoAmI | null; personas: Persona[] }) {
  if (!whoami || whoami.anonymous) {
    return (
      <span className="text-xs font-mono px-2.5 py-1 rounded-full border text-muted-foreground whitespace-nowrap">
        anonymous
      </span>
    )
  }
  const actor = actorHeaderValue(whoami.actor)
  const label =
    personas.find((p) => p.actor === actor)?.label ??
    (typeof whoami.actor === "string" ? whoami.actor.slice(0, 8) + "…" : String(whoami.actor))
  return (
    <span className="text-xs font-mono px-2.5 py-1 rounded-full border flex items-center gap-1.5 whitespace-nowrap">
      <span
        className={cn("size-2 rounded-full", whoami.known ? "bg-foreground" : "bg-destructive")}
      />
      {label}
      {whoami.known ? <Check size={12} /> : <span className="text-destructive">unknown</span>}
    </span>
  )
}

// /~personas exposes the anchor key in its native JSON scalar type. Studio's
// actor state is the exact textual value sent in X-Spock-Actor, so normalize at
// the API boundary before feeding values to the string-valued Select controls.
function normalizePersonas(body: unknown): Persona[] {
  if (!Array.isArray(body)) return []
  return body.flatMap((value) => {
    if (typeof value !== "object" || value === null) return []
    const { actor, label } = value as { actor?: unknown; label?: unknown }
    const normalizedActor = actorHeaderValue(actor)
    if (normalizedActor === null || typeof label !== "string") return []
    return [{ actor: normalizedActor, label }]
  })
}

function actorHeaderValue(actor: unknown): string | null {
  return typeof actor === "string" || typeof actor === "number" || typeof actor === "boolean"
    ? String(actor)
    : null
}

// --- status bar ------------------------------------------------------------
function StatusBar({ status }: { status: StatusContent }) {
  return (
    <footer className="flex items-center gap-3 px-3.5 border-t bg-background text-xs text-muted-foreground">
      <div className="flex items-center gap-2.5">{status.left}</div>
      <div className="ml-auto flex items-center gap-2">{status.right}</div>
    </footer>
  )
}
