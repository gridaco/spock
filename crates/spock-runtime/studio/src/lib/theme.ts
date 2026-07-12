// Dark mode via the shadcn `.dark` class on <html>. Default follows the OS,
// overridable by the toggle and remembered across reloads.
const KEY = "spock-studio-theme"

export function initTheme(): void {
  const saved = localStorage.getItem(KEY)
  const dark = saved
    ? saved === "dark"
    : window.matchMedia("(prefers-color-scheme: dark)").matches
  document.documentElement.classList.toggle("dark", dark)
}

export function toggleTheme(): boolean {
  const dark = !document.documentElement.classList.contains("dark")
  document.documentElement.classList.toggle("dark", dark)
  localStorage.setItem(KEY, dark ? "dark" : "light")
  return dark
}

export function isDark(): boolean {
  return document.documentElement.classList.contains("dark")
}
