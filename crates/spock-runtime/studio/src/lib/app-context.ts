import { createContext, useContext } from "react"
import type { ReactNode } from "react"
import type { Contract, Persona, Route } from "@/types"

export interface StatusContent {
  left?: ReactNode
  right?: ReactNode
}

export interface AppState {
  contract: Contract
  personas: Persona[]
  refreshPersonas: () => Promise<void>
  actor: string | null
  setActor: (a: string | null) => void
  route: Route
  navigate: (r: Route) => void
  // bumped by the refresh button so open views re-fetch under the current actor
  reloadKey: number
  reload: () => void
  setStatus: (s: StatusContent) => void
}

export const AppContext = createContext<AppState | null>(null)

export function useApp(): AppState {
  const ctx = useContext(AppContext)
  if (!ctx) throw new Error("useApp must be used within an AppContext provider")
  return ctx
}
