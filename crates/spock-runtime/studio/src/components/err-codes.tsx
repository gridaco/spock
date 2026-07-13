import { cn } from "@/lib/utils"

export interface ErrCode {
  code: string
  kind?: string
  refusal?: boolean
}

// Presentational only (no state) — the declared failure surface: errors[] with
// the minted refusals[] subset marked (✦). Refusals get a stronger neutral
// border; nothing is colored but genuine errors elsewhere.
export function ErrCodes({ codes }: { codes: ErrCode[] }) {
  if (!codes.length) return <span className="text-muted-foreground">—</span>
  return (
    <div className="flex flex-wrap gap-1.5" style={{ maxWidth: "74ch" }}>
      {codes.map((e) => (
        <code
          key={e.code}
          className={cn(
            "text-xs px-2 py-0.5 rounded border bg-muted/50 font-mono",
            e.refusal && "border-foreground/40",
          )}
          title={e.refusal ? "a declared refusal (spock_refuse)" : undefined}
        >
          {e.code}
          {e.kind ? <span className="text-muted-foreground"> · {e.kind}</span> : null}
          {e.refusal ? " ✦" : ""}
        </code>
      ))}
    </div>
  )
}
