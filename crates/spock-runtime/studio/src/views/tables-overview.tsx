import { Component } from "react"
import { Table2 } from "lucide-react"
import { AppContext } from "@/lib/app-context"
import type { AppState } from "@/lib/app-context"
import { Badge } from "@/components/ui/badge"
import { Card } from "@/components/ui/card"
import { Doc } from "@/components/doc"

export class TablesOverview extends Component {
  static contextType = AppContext
  declare context: AppState

  componentDidMount() {
    const n = this.context.contract.tables.length
    this.context.setStatus({
      left: (
        <span>
          <b className="text-foreground">{n}</b> tables
        </span>
      ),
    })
  }

  render() {
    const { contract, navigate } = this.context
    const n = contract.tables.length
    return (
      <div className="h-full overflow-y-auto">
        <div className="p-6">
          <h1 className="text-xl font-semibold tracking-tight">Tables</h1>
          <p className="text-sm text-muted-foreground mt-0.5 mb-4">
            {n} table{n === 1 ? "" : "s"} in the contract · click a row to open
          </p>
          <Card className="overflow-hidden p-0">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b text-xs uppercase tracking-wide text-muted-foreground">
                  <th className="text-left font-medium px-3 py-2 w-10">#</th>
                  <th className="text-left font-medium px-3 py-2">Name</th>
                  <th className="text-left font-medium px-3 py-2">Key</th>
                  <th className="text-left font-medium px-3 py-2">Fields</th>
                  <th className="text-left font-medium px-3 py-2">Uniques</th>
                  <th className="text-left font-medium px-3 py-2">Anchor</th>
                </tr>
              </thead>
              <tbody>
                {contract.tables.map((t, i) => (
                  <tr
                    key={t.name}
                    onClick={() => navigate({ kind: "table", name: t.name })}
                    className="border-b last:border-0 cursor-pointer hover:bg-muted/50"
                  >
                    <td className="px-3 py-2 text-muted-foreground font-mono">{i + 1}</td>
                    <td className="px-3 py-2">
                      <span className="flex items-center gap-2 font-mono">
                        <Table2 size={14} className="text-muted-foreground" />
                        {t.name}
                      </span>
                      <Doc text={t.doc} className="text-xs mt-1 line-clamp-2 max-w-md" />
                    </td>
                    <td className="px-3 py-2 font-mono">{t.key.join(", ")}</td>
                    <td className="px-3 py-2 font-mono">{t.fields.length}</td>
                    <td className="px-3 py-2 font-mono">{t.uniques?.length ?? 0}</td>
                    <td className="px-3 py-2">
                      {t.anchor ? (
                        <Badge variant="outline">auth</Badge>
                      ) : (
                        <span className="text-muted-foreground">—</span>
                      )}
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
