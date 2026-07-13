import { Component } from "react"
import { RefreshCw } from "lucide-react"

import { api, isErrorBody } from "@/lib/api"
import { AppContext } from "@/lib/app-context"
import type { AppState } from "@/lib/app-context"
import { coerce, fnSignature, isActorSensitive, readsActor, typeStr } from "@/lib/contract"
import { cn } from "@/lib/utils"
import type { FnDef } from "@/types"

import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Doc } from "@/components/doc"
import { ErrCodes } from "@/components/err-codes"

interface RunResult {
  status: number
  body: unknown
}

interface State {
  args: Record<string, string>
  result: RunResult | null
  running: boolean
}

export class FnRunner extends Component<{ name: string }, State> {
  static contextType = AppContext
  declare context: AppState
  state: State = { args: {}, result: null, running: false }
  private lastActor: string | null = null
  private lastReload = -1

  private fn(): FnDef | undefined {
    return this.context.contract.fns?.find((f) => f.name === this.props.name)
  }

  componentDidMount() {
    this.lastActor = this.context.actor
    this.lastReload = this.context.reloadKey
    const fn = this.fn()
    const n = fn?.params?.length ?? 0
    this.context.setStatus({
      left: (
        <span>
          {fn?.readonly ? "read fn" : "mut fn"} · {n} arg{n === 1 ? "" : "s"}
        </span>
      ),
    })
  }

  componentDidUpdate() {
    const { actor, reloadKey } = this.context
    if (actor !== this.lastActor || reloadKey !== this.lastReload) {
      this.lastActor = actor
      this.lastReload = reloadKey
      // a persona switch (or refresh) invalidates the shown answer — re-run to re-answer
      if (this.state.result !== null) this.setState({ result: null })
    }
  }

  private run = async () => {
    const fn = this.fn()
    if (!fn) return
    this.setState({ running: true })
    const payload: Record<string, unknown> = {}
    for (const p of fn.params ?? []) {
      const raw = this.state.args[p.name] ?? ""
      if (raw === "" && p.optional) continue
      payload[p.name] = coerce(raw, p.type)
    }
    let res: RunResult
    if (fn.readonly) {
      const qs = Object.entries(payload)
        .map(([k, v]) => `${encodeURIComponent(k)}=${encodeURIComponent(String(v))}`)
        .join("&")
      res = await api(
        `/rest/v1/rpc/${encodeURIComponent(fn.name)}${qs ? "?" + qs : ""}`,
        this.context.actor,
      )
    } else {
      res = await api(`/rest/v1/rpc/${encodeURIComponent(fn.name)}`, this.context.actor, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(payload),
      })
    }
    this.setState({ running: false, result: res })
  }

  render() {
    const fn = this.fn()
    if (!fn) return <div className="p-6 text-muted-foreground">function not found</div>
    const { actor, personas } = this.context
    const { args, result, running } = this.state
    const actorLabel = actor
      ? (personas.find((p) => p.actor === actor)?.label ?? "actor")
      : "anonymous"

    return (
      <div className="h-full overflow-y-auto">
        <div className="p-6 max-w-4xl">
          <h1 className="text-xl font-semibold tracking-tight flex items-center gap-2">
            {fn.name}
            <Badge variant="outline">{fn.readonly ? "read" : "mut"}</Badge>
            {isActorSensitive(fn) ? <Badge variant="outline">actor-sensitive</Badge> : null}
          </h1>
          <p className="text-sm text-muted-foreground mt-0.5">
            function · {fn.readonly ? "GET" : "POST"} <code>/rest/v1/rpc/{fn.name}</code>
          </p>
          <Doc text={fn.doc} className="text-sm mt-1.5 max-w-3xl" />
          <pre className="mt-2 rounded-md border bg-muted/40 px-3 py-2.5 font-mono text-[13.5px] overflow-x-auto">
            {fnSignature(fn)}
          </pre>

          {readsActor(fn) ? (
            <div className="mt-2 border-l-2 border-primary/50 bg-muted/40 rounded-r-md px-3.5 py-2.5 text-[13px] text-muted-foreground">
              This function reads who you're acting as — its result depends on the{" "}
              <b className="text-foreground">Actor</b> selector above. Switch persona and re-run
              to see it change.
            </div>
          ) : null}

          {(fn.errors ?? []).length ? (
            <>
              <h2 className="text-xs uppercase tracking-wide text-muted-foreground font-semibold mt-6 mb-2">
                Declared errors
              </h2>
              <ErrCodes
                codes={(fn.errors ?? []).map((code) => ({
                  code,
                  refusal: (fn.refusals ?? []).includes(code),
                }))}
              />
            </>
          ) : null}

          <h2 className="text-xs uppercase tracking-wide text-muted-foreground font-semibold mt-6 mb-2">
            Arguments
          </h2>
          {(fn.params ?? []).length ? (
            <div className="flex flex-col gap-2 max-w-xl">
              {(fn.params ?? []).map((p) => (
                <div key={p.name} className="grid grid-cols-[170px_1fr] items-center gap-x-2 gap-y-1">
                  <label htmlFor={`p_${p.name}`} className="font-mono text-[13px]">
                    {p.name}{" "}
                    <span className="text-muted-foreground">
                      {typeStr(p.type)}
                      {p.optional ? "?" : ""}
                    </span>
                  </label>
                  <Input
                    id={`p_${p.name}`}
                    value={args[p.name] ?? ""}
                    placeholder={p.optional ? "(optional)" : ""}
                    onChange={(e) =>
                      this.setState((s) => ({ args: { ...s.args, [p.name]: e.target.value } }))
                    }
                  />
                  <Doc text={p.doc} className="col-start-2 text-xs" />
                </div>
              ))}
            </div>
          ) : (
            <p className="text-sm text-muted-foreground">no parameters</p>
          )}

          <div className="flex items-center gap-3 mt-4">
            <Button onClick={() => void this.run()} disabled={running}>
              <RefreshCw size={14} /> Run as {actorLabel}
            </Button>
            <span className="text-xs text-muted-foreground">
              runs with the current <b className="text-foreground">Actor</b> persona
            </span>
          </div>

          {result ? <ResultBox result={result} /> : null}
        </div>
      </div>
    )
  }
}

function ResultBox({ result }: { result: RunResult }) {
  const ok = result.status >= 200 && result.status < 300 && !isErrorBody(result.body)
  const errCode = isErrorBody(result.body) ? result.body.error.code : null
  const errKind = isErrorBody(result.body) ? result.body.error.kind : null
  return (
    <div
      className={cn(
        "mt-3 rounded-md border p-3.5 bg-muted/30",
        ok ? "border-foreground/30" : "border-destructive",
      )}
    >
      <div className="font-mono text-xs text-muted-foreground mb-2">
        HTTP <span className="text-foreground font-semibold">{result.status}</span>
        {errCode ? (
          <>
            {" · "}
            <span className="text-destructive">{errCode}</span>
            {errKind ? " · " + errKind : ""}
          </>
        ) : null}
      </div>
      <pre className="font-mono text-[12.5px] whitespace-pre-wrap break-words">
        {JSON.stringify(result.body, null, 2)}
      </pre>
    </div>
  )
}
