# Spock for VS Code

Repository-native language support for the currently implemented Spock v0
syntax. It associates `*.spock` files with the Spock language and provides:

- TextMate highlighting for declarations, types, constraints, literals,
  contextual forms, comments, and strings;
- visible errors for reserved future keywords and uppercase identifiers;
- bracket pairing, quote pairing, comment toggling, and brace indentation.

This extension intentionally has no language server and no generated JavaScript.
It is a declarative first layer that can be tested against the Rust lexer's
canonical vocabulary. Diagnostics, completion, navigation, and formatting are
future LSP work.

## Try it locally

Open `editors/vscode-spock` as the extension-development folder in VS Code and
launch an Extension Development Host (Run > Start Debugging). Open any accepted
example such as `examples/instagram/v0.spock` in that window.

For a packaged installation, run `npx @vscode/vsce package` in this directory,
then install the generated VSIX with `code --install-extension <file>.vsix`.

Always validate language behavior with the actual compiler as well:

```sh
cargo run --locked -p spock-cli -- check examples/instagram/v0.spock
```
