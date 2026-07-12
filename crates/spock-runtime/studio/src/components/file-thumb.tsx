// A preview of a stored file (RFD 0018): the signed download URL rendered as an
// image, falling back to a generic file glyph for non-images or objects with no
// servable bytes yet (pending). Self-contained — it mints its own signed URL
// under the current persona.

import { useEffect, useState } from "react"
import { File as FileIcon } from "lucide-react"

import { useApp } from "@/lib/app-context"
import { signDownload } from "@/lib/storage"
import { cn } from "@/lib/utils"

export function FileThumb({
  id,
  size = 24,
  className,
}: {
  id: string
  size?: number
  className?: string
}) {
  const app = useApp()
  const [url, setUrl] = useState<string | null>(null)
  const [broken, setBroken] = useState(false)

  useEffect(() => {
    let live = true
    setBroken(false)
    setUrl(null)
    void signDownload(id, app.actor).then((u) => {
      if (live) setUrl(u)
    })
    return () => {
      live = false
    }
  }, [id, app.actor])

  if (url && !broken) {
    return (
      <img
        src={url}
        alt=""
        onError={() => setBroken(true)}
        style={{ height: size, width: size }}
        className={cn("rounded object-cover border bg-muted", className)}
      />
    )
  }
  return (
    <span
      style={{ height: size, width: size }}
      className={cn(
        "flex items-center justify-center rounded border bg-muted text-muted-foreground",
        className,
      )}
    >
      <FileIcon size={Math.max(12, Math.round(size * 0.5))} />
    </span>
  )
}
