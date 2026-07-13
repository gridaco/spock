import { StrictMode } from "react"
import { createRoot } from "react-dom/client"
// index.css owns the cascade-layer order and imports react-data-grid's stylesheet
// into the correctly-ordered `rdg` layer (see the @layer note there).
import "./index.css"
import { initTheme } from "@/lib/theme"
import App from "@/app"

initTheme()

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
)
