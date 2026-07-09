//! Diagnostics: stable codes, messages, spans (docs/spec/v0.md §4).

use crate::span::{line_col, Span};

#[derive(Clone, Debug)]
pub struct Diagnostic {
    /// Stable code, e.g. "E003" (checker) or "L001" (lexer/parser).
    pub code: &'static str,
    pub message: String,
    pub span: Span,
}

impl Diagnostic {
    pub fn new(code: &'static str, message: impl Into<String>, span: Span) -> Self {
        Diagnostic {
            code,
            message: message.into(),
            span,
        }
    }

    /// Render as `path:line:col: error[CODE]: message`.
    pub fn render(&self, source: &str, path: &str) -> String {
        let (line, col) = line_col(source, self.span.start);
        format!(
            "{path}:{line}:{col}: error[{code}]: {msg}",
            code = self.code,
            msg = self.message
        )
    }
}
