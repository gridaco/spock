import { Component } from "react"
import type { ReactNode } from "react"
import { AppContext } from "@/lib/app-context"
import type { AppState } from "@/lib/app-context"
import { readsActor } from "@/lib/contract"
import { Badge } from "@/components/ui/badge"
import { Card } from "@/components/ui/card"
import { ErrCodes } from "@/components/err-codes"

// The surface ledger (RFD 0015 §8): the v0 slice of the exposure surface,
// rendered from /~contract — it SHOWS the ungoverned floor, never fakes it.
export class Overview extends Component {
  static contextType = AppContext
  declare context: AppState

  componentDidMount() {
    this.context.setStatus({ left: <span>exposure surface · read-only</span> })
  }

  render() {
    const { contract, personas } = this.context
    const anchor = contract.tables.find((t) => t.anchor)
    const meCols: string[] = []
    contract.tables.forEach((t) =>
      t.fields.forEach((f) => {
        if (f.default?.kind === "actor") {
          const to = f.type.kind === "ref" ? (f.type.table ?? "?") : f.type.kind
          meCols.push(`${t.name}.${f.name} → ${to}`)
        }
      }),
    )
    const actorFns = (contract.fns ?? []).filter(readsActor).map((f) => f.name)

    return (
      <div className="h-full overflow-y-auto">
        <div className="p-6 max-w-5xl">
          <h1 className="text-xl font-semibold tracking-tight">Surface ledger</h1>
          <p className="text-sm text-muted-foreground mt-0.5">
            the v0 slice of the exposure surface, rendered from <code>/~contract</code> — role × via
            arrives with v1 governance
          </p>

          <SectionLabel>Identity</SectionLabel>
          <Card className="p-4 text-sm">
            {anchor ? (
              <div className="text-muted-foreground">
                auth anchor: <b className="text-foreground">{anchor.name}</b> · key{" "}
                <span className="font-mono">{anchor.key.join(", ")}</span> ·{" "}
                <b className="text-foreground">{personas.length}</b> persona
                {personas.length === 1 ? "" : "s"} seeded
              </div>
            ) : (
              <div className="text-muted-foreground">
                no <b className="text-foreground">auth table</b> — impersonation unavailable
              </div>
            )}
          </Card>

          <SectionLabel>
            Server-stamped identity columns <Badge variant="outline">= me</Badge>
          </SectionLabel>
          <Card className="p-4">
            {meCols.length ? (
              <>
                <div className="flex flex-wrap gap-1.5">
                  {meCols.map((m) => (
                    <code
                      key={m}
                      className="text-xs px-2 py-0.5 rounded border bg-muted/50 font-mono"
                    >
                      {m}
                    </code>
                  ))}
                </div>
                <p className="text-sm text-muted-foreground mt-2.5">
                  Stamped from the current actor, unforgeable on the floor —{" "}
                  <b className="text-foreground">provenance, not governance</b>.
                </p>
              </>
            ) : (
              <span className="text-muted-foreground text-sm">none</span>
            )}
          </Card>

          <SectionLabel>
            Actor-sensitive functions <Badge variant="outline">heuristic</Badge>
          </SectionLabel>
          <Card className="p-4">
            {actorFns.length ? (
              <>
                <div className="flex flex-wrap gap-1.5">
                  {actorFns.map((n) => (
                    <code
                      key={n}
                      className="text-xs px-2 py-0.5 rounded border bg-muted/50 font-mono"
                    >
                      {n}
                    </code>
                  ))}
                </div>
                <p className="text-sm text-muted-foreground mt-2.5">
                  Bodies that reference <code>spock_actor()</code> (a body scan, not an authoritative
                  contract bit). These re-answer under impersonation.
                </p>
              </>
            ) : (
              <span className="text-muted-foreground text-sm">none detected</span>
            )}
          </Card>

          {anchor ? (
            <div className="mt-4 border-l-2 border-muted-foreground/50 bg-muted/40 rounded-r-md px-4 py-3 text-sm text-muted-foreground">
              <b className="text-foreground">⚠ ungoverned floor write — no guard.</b> The floor
              (auto-derived GraphQL/REST writes) stays actor-blind in v0: only <code>= me</code>{" "}
              columns are server-stamped. The seam is preparatory, not protective, until v1{" "}
              <code>policy</code>. Studio shows this rather than faking governance the language does
              not yet have.
            </div>
          ) : null}

          <SectionLabel>Per-op outcomes</SectionLabel>
          <p className="text-sm text-muted-foreground -mt-1 mb-2">
            every declared failure, from the contract — <code>errors[]</code> with the minted{" "}
            <code>refusals[]</code> subset marked
          </p>
          <Card className="overflow-hidden p-0">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b text-xs uppercase tracking-wide text-muted-foreground">
                  <th className="text-left font-medium px-3 py-2">Operation</th>
                  <th className="text-left font-medium px-3 py-2">Kind</th>
                  <th className="text-left font-medium px-3 py-2">Outcomes</th>
                </tr>
              </thead>
              <tbody>
                {contract.tables.map((t) => (
                  <tr key={t.name} className="border-b last:border-0 align-top">
                    <td className="px-3 py-2 font-mono">{t.name}</td>
                    <td className="px-3 py-2 text-muted-foreground">table</td>
                    <td className="px-3 py-2">
                      <ErrCodes codes={t.errors.map((e) => ({ code: e.code, kind: e.kind }))} />
                    </td>
                  </tr>
                ))}
                {(contract.fns ?? []).map((f) => (
                  <tr key={f.name} className="border-b last:border-0 align-top">
                    <td className="px-3 py-2 font-mono">{f.name}</td>
                    <td className="px-3 py-2 text-muted-foreground">
                      {f.readonly ? "read fn" : "mut fn"}
                    </td>
                    <td className="px-3 py-2">
                      <ErrCodes
                        codes={(f.errors ?? []).map((code) => ({
                          code,
                          refusal: (f.refusals ?? []).includes(code),
                        }))}
                      />
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </Card>
        </div>
      </div>
    )
  }
}

function SectionLabel({ children }: { children: ReactNode }) {
  return (
    <h2 className="text-xs uppercase tracking-wide text-muted-foreground font-semibold mt-6 mb-2.5 flex items-center gap-2">
      {children}
    </h2>
  )
}
