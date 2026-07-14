// Select values share one string namespace, while an auth anchor may itself
// use any string as a key. Tag actor options structurally so no legal key can
// collide with Studio's anonymous choice.
export const ANONYMOUS_ACTOR_SELECT_VALUE = '["anonymous"]'

export function actorSelectValue(actor: string | null): string {
  return actor === null
    ? ANONYMOUS_ACTOR_SELECT_VALUE
    : JSON.stringify(["actor", actor])
}

export function actorFromSelectValue(value: unknown): string | null {
  if (typeof value !== "string") return null

  let decoded: unknown
  try {
    decoded = JSON.parse(value)
  } catch {
    return null
  }

  return Array.isArray(decoded) &&
    decoded.length === 2 &&
    decoded[0] === "actor" &&
    typeof decoded[1] === "string"
    ? decoded[1]
    : null
}
