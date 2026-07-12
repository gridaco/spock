// Every request the console makes is same-origin and carries the impersonation
// header when a persona is selected — this is the whole point of studio.

export interface ApiResult {
  status: number
  // deliberately loose: bodies are contract-shaped or an { error } envelope
  body: unknown
}

export async function api(
  path: string,
  actor: string | null,
  opts: RequestInit = {},
): Promise<ApiResult> {
  const headers = new Headers(opts.headers)
  if (actor) headers.set("X-Spock-Actor", actor)
  let res: Response
  let text = ""
  try {
    res = await fetch(path, { ...opts, headers })
    text = await res.text()
  } catch (e) {
    return { status: 0, body: { error: { code: "network", message: String(e) } } }
  }
  let body: unknown
  try {
    body = text ? JSON.parse(text) : null
  } catch {
    body = text
  }
  return { status: res.status, body }
}

export function isErrorBody(
  body: unknown,
): body is { error: { code: string; kind?: string; message?: string } } {
  return (
    typeof body === "object" &&
    body !== null &&
    "error" in body &&
    typeof (body as { error: unknown }).error === "object"
  )
}
