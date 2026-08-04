import { defineConfig, markdown } from "sourcey";

export default defineConfig({
  name: "Spock",
  siteUrl: "https://spock.sh",
  prettyUrls: "slash",
  repo: "https://github.com/gridaco/spock",
  editBranch: "main",
  navigation: {
    tabs: [
      {
        tab: "Documentation",
        slug: "",
        source: markdown({
          groups: [
            {
              group: "Getting started",
              pages: [
                "docs/start/install",
                "docs/start/quickstart",
                "docs/start/tutorial",
              ],
            },
            {
              group: "Language guides",
              pages: [
                "docs/language/actor",
                "docs/language/derived-api",
                "docs/language/functions",
                "docs/language/seed",
                "docs/language/tables",
              ],
            },
            {
              group: "Reference",
              pages: [
                "docs/reference/cli",
                "docs/reference/errors",
                "docs/reference/http",
                "docs/reference/spock-toml",
              ],
            },
            {
              group: "Project guides",
              pages: ["docs/status", "docs/uhura", "docs/examples"],
            },
            {
              group: "Normative specifications",
              pages: ["docs/spec/v0", "docs/spec/graphql"],
            },
            {
              group: "Governance",
              pages: [
                "docs/governance/design-principles",
                "docs/governance/language-change-process",
                "docs/governance/language-design-committee",
                "docs/governance/commonly-declined",
              ],
            },
          ],
        }),
      },
    ],
  },
});
