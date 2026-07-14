# Spock TextMate grammar

This directory is the repository's canonical TextMate grammar for the currently
implemented Spock v0 syntax. The tiny static VS Code manifest exists only to
load that grammar locally; it is not the published Spock language-support
product.

It associates `*.spock` files with language ID `spock` and provides:

- conventional TextMate scopes for declarations, names, parameters, types,
  constraints, literals, comments, strings, and punctuation;
- contextual highlighting only for `unchecked sql(...)` and soft `file(...)`
  call forms;
- invalid scopes for future-reserved words, uppercase identifiers, and unknown
  string escapes;
- comment toggling, pairs, indentation, word selection, and folding markers.

There is no runtime JavaScript or TypeScript, dependency, lockfile, generated
grammar source, packaging workflow, custom theme, or language server here. The
Rust compiler remains the source of diagnostics and semantic validation.

## Load and inspect it locally

1. Open `editors/vscode` as a VS Code workspace.
2. Run **Start Debugging** and choose **Run Spock TextMate Loader**.
3. In the Extension Development Host, open `test/corpus.spock` or an accepted
   repository example.
4. Run **Developer: Inspect Editor Tokens and Scopes** and place the cursor on
   representative tokens.
5. Repeat with built-in light and dark themes. Theme selection is deliberately
   outside this grammar.

The corpus covers the active lexical families and is also compiled by the Rust
test suite. For larger manual smoke inputs, use:

- `examples/filter-lab/schema.spock`
- `examples/instagram-poc/app.spock`
- `examples/instagram/v0.spock`

Do not use `examples/instagram/v1.spock` or `docs/rfd/0000-vision.spock` as
accepted-language inputs; they intentionally contain speculative syntax.

## Validate behavior

Always validate language behavior with the actual compiler as well:

```sh
cargo test -p spock-lang
cargo run --locked -p spock-cli -- check editors/vscode/test/corpus.spock
```

TextMate highlighting is intentionally fast and lossy. Diagnostics, completion,
hover, symbols, navigation, rename, formatting, and semantic tokens belong to a
future standard Spock LSP and are outside this loader.
