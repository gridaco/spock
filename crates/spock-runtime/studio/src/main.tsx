import { StrictMode } from "react"
import { createRoot } from "react-dom/client"
// react-data-grid styles first, so our .rdg token overrides in index.css win
import "react-data-grid/lib/styles.css"
import "./index.css"
import { initTheme } from "@/lib/theme"
import App from "@/app"

initTheme()

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
)
