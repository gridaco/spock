import { Component } from "react"
import { Code2 } from "lucide-react"
import { AppContext } from "@/lib/app-context"
import type { AppState } from "@/lib/app-context"
import { isActorSensitive, returnStr, typeStr } from "@/lib/contract"
import { Badge } from "@/components/ui/badge"
import { Card } from "@/components/ui/card"
import { Doc } from "@/components/doc"
import { ErrCodes } from "@/components/err-codes"

export class FunctionsOverview extends Component {
  static contextType = AppContext
  declare context: AppState

  componentDidMount() {
    const n = (this.context.contract.fns ?? []).length
    this.context.setStatus({
      left: (
        <span>
          <b className="text-foreground">{n}</b> functions
        </span>
      ),
    })
  }

  render() {
    const { contract, navigate } = this.context
    const fns = contract.fns ?? []
    return (
      <div className="h-full overflow-y-auto">
        <div className="p-6">
          <h1 className="text-xl font-semibold tracking-tight">Functions</h1>
          <p className="text-sm text-muted-foreground mt-0.5 mb-4">
            {fns.length} function{fns.length === 1 ? "" : "s"} · read (GET) &amp; mut (POST) over{" "}
            <code>/rest/v1/rpc</code> · click a row to open
          </p>
          <Card className="overflow-hidden p-0">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b text-xs uppercase tracking-wide text-muted-foreground">
                  <th className="text-left font-medium px-3 py-2 w-10">#</th>
                  <th className="text-left font-medium px-3 py-2">Name</th>
                  <th className="text-left font-medium px-3 py-2">Kind</th>
                  <th className="text-left font-medium px-3 py-2">Arguments</th>
                  <th className="text-left font-medium px-3 py-2">Returns</th>
                  <th className="text-left font-medium px-3 py-2">Outcomes</th>
                </tr>
              </thead>
              <tbody>
                {fns.map((f, i) => (
                  <tr
                    key={f.name}
                    onClick={() => navigate({ kind: "fn", name: f.name })}
                    className="border-b last:border-0 cursor-pointer hover:bg-muted/50 align-top"
                  >
                    <td className="px-3 py-2 text-muted-foreground font-mono">{i + 1}</td>
                    <td className="px-3 py-2">
                      <span className="flex items-center gap-2 font-mono">
                        <Code2 size={14} className="text-muted-foreground" />
                        {f.name}
                      </span>
                      <Doc text={f.doc} className="text-xs mt-1 line-clamp-2 max-w-md" />
                    </td>
                    <td className="px-3 py-2 whitespace-nowrap">
                      <span className="flex items-center gap-1">
                        <Badge variant="outline">{f.readonly ? "read" : "mut"}</Badge>
                        {isActorSensitive(f) ? <Badge variant="outline">me?</Badge> : null}
                      </span>
                    </td>
                    <td className="px-3 py-2 font-mono">
                      {(f.params ?? [])
                        .map((p) => `${p.name}: ${typeStr(p.type)}${p.optional ? "?" : ""}`)
                        .join(", ") || <span className="text-muted-foreground">—</span>}
                    </td>
                    <td className="px-3 py-2 font-mono">{returnStr(f.returns)}</td>
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
