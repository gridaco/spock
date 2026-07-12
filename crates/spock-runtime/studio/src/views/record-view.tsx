import { Component } from "react"
import { AppContext } from "@/lib/app-context"
import type { AppState } from "@/lib/app-context"
import { typeStr } from "@/lib/contract"
import type { RecordDef } from "@/types"
import { Badge } from "@/components/ui/badge"
import { Card } from "@/components/ui/card"

// App keys each view by route.name, so this remounts (and componentDidMount
// re-runs) when the selected record changes — no componentDidUpdate needed.
export class RecordView extends Component<{ name: string }> {
  static contextType = AppContext
  declare context: AppState

  private record(): RecordDef | undefined {
    return this.context.contract.records?.find((r) => r.name === this.props.name)
  }

  componentDidMount() {
    const rec = this.record()
    this.context.setStatus({
      left: rec ? (
        <span>
          <b className="text-foreground">{rec.fields.length}</b> fields
        </span>
      ) : null,
    })
  }

  render() {
    const rec = this.record()
    if (!rec) return <div className="p-6 text-muted-foreground">record not found</div>
    return (
      <div className="h-full overflow-y-auto">
        <div className="p-6 max-w-3xl">
          <h1 className="text-xl font-semibold tracking-tight">{rec.name}</h1>
          <p className="text-sm text-muted-foreground mt-0.5 mb-4">
            record · a named fn-return shape (scalars only)
          </p>
          <Card className="overflow-hidden p-0">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b text-xs uppercase tracking-wide text-muted-foreground">
                  <th className="text-left font-medium px-3 py-2 w-10">#</th>
                  <th className="text-left font-medium px-3 py-2">Field</th>
                  <th className="text-left font-medium px-3 py-2">Type</th>
                </tr>
              </thead>
              <tbody>
                {rec.fields.map((f, i) => (
                  <tr key={f.name} className="border-b last:border-0">
                    <td className="px-3 py-2 text-muted-foreground font-mono">{i + 1}</td>
                    <td className="px-3 py-2 font-mono">{f.name}</td>
                    <td className="px-3 py-2 font-mono">
                      {typeStr(f.type)}
                      {f.optional ? (
                        <Badge variant="outline" className="ml-2">
                          optional
                        </Badge>
                      ) : null}
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
