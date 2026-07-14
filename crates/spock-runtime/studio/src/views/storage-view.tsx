// The Storage page (RFD 0018) browses the `storage_object` table: every uploaded
// file is a card, with an upload button
// that drives the same signed-URL gate any client uses. Standalone uploads here
// are unattached, so the orphan sweep reclaims them unless a row references
// them — the note says so.

import { Component, createRef } from "react"
import type { ChangeEvent, ReactNode } from "react"
import { Download, RefreshCw, Upload } from "lucide-react"

import { api } from "@/lib/api"
import { AppContext } from "@/lib/app-context"
import type { AppState } from "@/lib/app-context"
import { useApp } from "@/lib/app-context"
import { signDownload, storageTable, uploadFile } from "@/lib/storage"
import { cn } from "@/lib/utils"

import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card } from "@/components/ui/card"
import { FileThumb } from "@/components/file-thumb"

type Row = Record<string, unknown>

interface State {
  objects: Row[]
  loading: boolean
  uploading: boolean
  err: string | null
}

export class StorageView extends Component<Record<string, never>, State> {
  static contextType = AppContext
  declare context: AppState
  state: State = { objects: [], loading: false, uploading: false, err: null }
  private lastActor: string | null = null
  private lastReload = -1
  private inputRef = createRef<HTMLInputElement>()

  componentDidMount() {
    this.lastActor = this.context.actor
    this.lastReload = this.context.reloadKey
    void this.load()
    this.pushStatus()
  }

  componentDidUpdate(_prev: Record<string, never>, prevState: State) {
    const { actor, reloadKey } = this.context
    if (actor !== this.lastActor || reloadKey !== this.lastReload) {
      this.lastActor = actor
      this.lastReload = reloadKey
      void this.load()
    }
    if (prevState.objects !== this.state.objects) this.pushStatus()
  }

  private load = async () => {
    this.setState({ loading: true, err: null })
    const res = await api("/rest/v1/storage_object?limit=200", this.context.actor)
    if (res.status !== 200) {
      this.setState({ loading: false, err: `HTTP ${res.status}`, objects: [] })
      return
    }
    const body = res.body as { rows?: Row[] }
    this.setState({ loading: false, objects: body.rows ?? [] })
  }

  private pushStatus() {
    const n = this.state.objects.length
    this.context.setStatus({
      left: (
        <span>
          <b className="text-foreground">{n}</b> object{n === 1 ? "" : "s"} · read-only
        </span>
      ),
    })
  }

  private onPick = async (e: ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0]
    if (this.inputRef.current) this.inputRef.current.value = ""
    if (!file) return
    this.setState({ uploading: true, err: null })
    try {
      await uploadFile(file, this.context.actor)
      await this.load()
    } catch (ex) {
      this.setState({ err: ex instanceof Error ? ex.message : String(ex) })
    } finally {
      this.setState({ uploading: false })
    }
  }

  render() {
    const table = storageTable(this.context.contract)
    if (!table) {
      return <div className="p-6 text-muted-foreground">storage is not enabled for this contract</div>
    }
    const { objects, loading, uploading, err } = this.state
    return (
      <div className="h-full flex flex-col min-h-0">
        <div className="px-6 pt-5">
          <h1 className="text-xl font-semibold tracking-tight">Storage</h1>
          <p className="text-sm text-muted-foreground mt-0.5">
            the <code>storage_object</code> table — every uploaded file and its metadata
          </p>
          <Note>
            Uploads here are <b className="text-foreground">unattached</b>. A file becomes
            durable once a row references it (upload directly on a table's file column);
            unreferenced files are cleaned up automatically. Owner is set from the selected
            actor.
          </Note>
          <div className="flex items-center gap-2.5 my-3">
            <input ref={this.inputRef} type="file" hidden onChange={this.onPick} />
            <Button size="sm" onClick={() => this.inputRef.current?.click()} disabled={uploading}>
              <Upload size={14} /> {uploading ? "Uploading…" : "Upload file"}
            </Button>
            <div className="flex-1" />
            <Button size="sm" variant="outline" onClick={() => void this.load()}>
              <RefreshCw size={14} /> Refresh
            </Button>
          </div>
          {err ? <div className="text-destructive text-sm mb-2">{err}</div> : null}
        </div>

        <div className="flex-1 min-h-0 overflow-y-auto px-6 pb-6">
          {objects.length === 0 && !loading ? (
            <div className="p-10 text-center text-muted-foreground text-sm">
              no files yet — upload one above
            </div>
          ) : (
            <div className="grid gap-3" style={{ gridTemplateColumns: "repeat(auto-fill, minmax(200px, 1fr))" }}>
              {objects.map((o) => (
                <ObjectCard key={String(o.id)} obj={o} />
              ))}
            </div>
          )}
        </div>
      </div>
    )
  }
}

function ObjectCard({ obj }: { obj: Row }) {
  const app = useApp()
  const id = String(obj.id)
  const state = String(obj.state ?? "")
  const contentType = (obj.content_type ?? null) as string | null
  const size = (obj.size ?? null) as number | null
  const name = (obj.name ?? null) as string | null
  const committed = state === "committed"

  const download = async () => {
    const url = await signDownload(id, app.actor)
    if (url) window.open(url, "_blank", "noopener")
  }

  return (
    <Card className="p-3 flex flex-col gap-2 overflow-hidden">
      <div className="flex items-center justify-center h-24 rounded bg-muted/40">
        {committed ? (
          <FileThumb id={id} size={80} className="h-20 w-20" />
        ) : (
          <span className="text-xs text-muted-foreground italic">pending upload</span>
        )}
      </div>
      <div className="min-w-0">
        <div className="font-mono text-[12px] truncate" title={name ?? id}>
          {name ?? id}
        </div>
        <div className="flex items-center gap-1.5 mt-1 flex-wrap">
          <Badge variant={committed ? "outline" : "secondary"} className="text-[10px]">
            {state}
          </Badge>
          <span className="text-[11px] text-muted-foreground truncate">
            {contentType ?? "—"} · {size != null ? humanSize(size) : "—"}
          </span>
        </div>
      </div>
      {committed ? (
        <Button size="sm" variant="outline" className="h-7 text-[12px]" onClick={download}>
          <Download size={13} /> Download
        </Button>
      ) : null}
    </Card>
  )
}

function humanSize(n: number): string {
  if (n < 1024) return `${n} B`
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`
  return `${(n / (1024 * 1024)).toFixed(1)} MB`
}

function Note({ children }: { children: ReactNode }) {
  return (
    <div
      className={cn(
        "border-l-2 rounded-r-md px-3.5 py-2.5 text-[13px] text-muted-foreground mt-2 bg-muted/40",
        "border-primary/50",
      )}
    >
      {children}
    </div>
  )
}
