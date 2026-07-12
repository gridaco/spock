import path from "node:path"
import { defineConfig } from "vite"
import react from "@vitejs/plugin-react"
import tailwindcss from "@tailwindcss/vite"

// The studio console is compiled to static assets, embedded in the spock binary
// (rust-embed) and served at /~studio — so every asset URL is rooted there via
// `base`. In dev, `pnpm dev` runs a Vite server and proxies the meta/data
// endpoints to a running `spock run` on :4000, so contract fetches and the
// X-Spock-Actor impersonation header work exactly as they do embedded.
export default defineConfig({
  base: "/~studio/",
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: { "@": path.resolve(import.meta.dirname, "./src") },
  },
  build: {
    outDir: "dist",
    emptyOutDir: true,
  },
  server: {
    proxy: {
      "/~contract": "http://127.0.0.1:4000",
      "/~personas": "http://127.0.0.1:4000",
      "/~whoami": "http://127.0.0.1:4000",
      "/rest": "http://127.0.0.1:4000",
      "/graphql": "http://127.0.0.1:4000",
      "/storage": "http://127.0.0.1:4000",
    },
  },
})
