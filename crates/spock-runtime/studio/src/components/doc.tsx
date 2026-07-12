import { cn } from "@/lib/utils"

// A rendered `///` / `//!` doc comment (RFD 0016) — the author's prose,
// carried verbatim from /~contract. Multi-line docs keep their line breaks
// (`whitespace-pre-line`). Renders nothing when the entity is undocumented,
// so callers can drop it in unconditionally.
export function Doc({ text, className }: { text?: string | null; className?: string }) {
  if (!text) return null
  return <p className={cn("text-muted-foreground whitespace-pre-line", className)}>{text}</p>
}
