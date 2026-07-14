# Repository-local skills

This directory is reserved for skills that help agents work on the Spock
repository itself. Put contributor-facing workflows here: language design,
compiler implementation, specification maintenance, release engineering, and
similar repository concerns.

For example, a future `language-design/` skill could guide grammar and semantic
changes, compatibility analysis, and the accompanying spec or RFD updates.
Each repository-local skill should live in its own directory with a `SKILL.md`.
Mark it with `metadata.internal: true` so public skill installers do not expose
it as part of Spock's user-facing catalog.

Do not link or copy `../../skills/spock-lang` into this directory. That skill is
for people building applications with Spock, not for developing Spock itself.
