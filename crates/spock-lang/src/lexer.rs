//! Lexer (docs/spec/v0.md §2).

use crate::diag::Diagnostic;
use crate::span::Span;

#[derive(Clone, Debug, PartialEq)]
pub enum TokenKind {
    Ident(String),
    Str(String),
    Int(i64),

    // punctuation
    LBrace,
    RBrace,
    LParen,
    RParen,
    Colon,
    Comma,
    Question,
    Eq,

    // active keywords (§2.3)
    KwTable,
    KwKey,
    KwUnique,
    KwSeed,
    KwOn,
    KwDelete,
    KwRestrict,
    KwCascade,
    KwText,
    KwInt,
    KwBool,
    KwTimestamp,
    KwUuid,
    KwAuto,
    KwNow,
    KwTrue,
    KwFalse,

    Eof,
}

impl TokenKind {
    /// Human-readable name for error messages.
    pub fn describe(&self) -> String {
        match self {
            TokenKind::Ident(name) => format!("identifier `{name}`"),
            TokenKind::Str(_) => "string literal".to_string(),
            TokenKind::Int(_) => "integer literal".to_string(),
            TokenKind::LBrace => "`{`".to_string(),
            TokenKind::RBrace => "`}`".to_string(),
            TokenKind::LParen => "`(`".to_string(),
            TokenKind::RParen => "`)`".to_string(),
            TokenKind::Colon => "`:`".to_string(),
            TokenKind::Comma => "`,`".to_string(),
            TokenKind::Question => "`?`".to_string(),
            TokenKind::Eq => "`=`".to_string(),
            TokenKind::Eof => "end of file".to_string(),
            kw => format!("keyword `{}`", keyword_text(kw)),
        }
    }
}

fn keyword_text(kind: &TokenKind) -> &'static str {
    match kind {
        TokenKind::KwTable => "table",
        TokenKind::KwKey => "key",
        TokenKind::KwUnique => "unique",
        TokenKind::KwSeed => "seed",
        TokenKind::KwOn => "on",
        TokenKind::KwDelete => "delete",
        TokenKind::KwRestrict => "restrict",
        TokenKind::KwCascade => "cascade",
        TokenKind::KwText => "text",
        TokenKind::KwInt => "int",
        TokenKind::KwBool => "bool",
        TokenKind::KwTimestamp => "timestamp",
        TokenKind::KwUuid => "uuid",
        TokenKind::KwAuto => "auto",
        TokenKind::KwNow => "now",
        TokenKind::KwTrue => "true",
        TokenKind::KwFalse => "false",
        _ => unreachable!("not a keyword"),
    }
}

/// Keywords reserved for future versions (§2.3): using one is L005.
const RESERVED: &[&str] = &[
    "fn",
    "view",
    "role",
    "policy",
    "error",
    "state",
    "record",
    "extern",
    "derived",
    "protected",
    "module",
    "enum",
    "expect",
    "transition",
    "upsert",
    "match",
    "with",
];

fn keyword(word: &str) -> Option<TokenKind> {
    Some(match word {
        "table" => TokenKind::KwTable,
        "key" => TokenKind::KwKey,
        "unique" => TokenKind::KwUnique,
        "seed" => TokenKind::KwSeed,
        "on" => TokenKind::KwOn,
        "delete" => TokenKind::KwDelete,
        "restrict" => TokenKind::KwRestrict,
        "cascade" => TokenKind::KwCascade,
        "text" => TokenKind::KwText,
        "int" => TokenKind::KwInt,
        "bool" => TokenKind::KwBool,
        "timestamp" => TokenKind::KwTimestamp,
        "uuid" => TokenKind::KwUuid,
        "auto" => TokenKind::KwAuto,
        "now" => TokenKind::KwNow,
        "true" => TokenKind::KwTrue,
        "false" => TokenKind::KwFalse,
        _ => return None,
    })
}

#[derive(Clone, Debug)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

pub fn lex(source: &str) -> Result<Vec<Token>, Diagnostic> {
    let bytes = source.as_bytes();
    let mut tokens = Vec::new();
    let mut i = 0;

    while i < bytes.len() {
        let b = bytes[i];

        // whitespace
        if b.is_ascii_whitespace() {
            i += 1;
            continue;
        }

        // comments: // to end of line
        if b == b'/' && bytes.get(i + 1) == Some(&b'/') {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }

        let start = i;

        // punctuation
        let punct = match b {
            b'{' => Some(TokenKind::LBrace),
            b'}' => Some(TokenKind::RBrace),
            b'(' => Some(TokenKind::LParen),
            b')' => Some(TokenKind::RParen),
            b':' => Some(TokenKind::Colon),
            b',' => Some(TokenKind::Comma),
            b'?' => Some(TokenKind::Question),
            b'=' => Some(TokenKind::Eq),
            _ => None,
        };
        if let Some(kind) = punct {
            i += 1;
            tokens.push(Token {
                kind,
                span: Span::new(start, i),
            });
            continue;
        }

        // string literal
        if b == b'"' {
            i += 1;
            let mut value = String::new();
            loop {
                match bytes.get(i) {
                    None | Some(b'\n') => {
                        return Err(Diagnostic::new(
                            "L003",
                            "unterminated string literal",
                            Span::new(start, i),
                        ));
                    }
                    Some(b'"') => {
                        i += 1;
                        break;
                    }
                    Some(b'\\') => {
                        let esc = bytes.get(i + 1);
                        match esc {
                            Some(b'"') => value.push('"'),
                            Some(b'\\') => value.push('\\'),
                            Some(b'n') => value.push('\n'),
                            Some(b't') => value.push('\t'),
                            _ => {
                                return Err(Diagnostic::new(
                                    "L003",
                                    "unknown escape sequence in string literal",
                                    Span::new(i, i + 2),
                                ));
                            }
                        }
                        i += 2;
                    }
                    Some(_) => {
                        // consume one full UTF-8 character
                        let ch = source[i..].chars().next().expect("in-bounds char");
                        value.push(ch);
                        i += ch.len_utf8();
                    }
                }
            }
            tokens.push(Token {
                kind: TokenKind::Str(value),
                span: Span::new(start, i),
            });
            continue;
        }

        // integer literal (minus only immediately before digits)
        if b.is_ascii_digit() || (b == b'-' && bytes.get(i + 1).is_some_and(u8::is_ascii_digit)) {
            i += 1;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            let text = &source[start..i];
            let value: i64 = text.parse().map_err(|_| {
                Diagnostic::new(
                    "L004",
                    format!("integer literal `{text}` does not fit in 64 bits"),
                    Span::new(start, i),
                )
            })?;
            tokens.push(Token {
                kind: TokenKind::Int(value),
                span: Span::new(start, i),
            });
            continue;
        }

        // identifier / keyword
        if b.is_ascii_lowercase() || b == b'_' {
            i += 1;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            let word = &source[start..i];
            if word.bytes().any(|c| c.is_ascii_uppercase()) {
                return Err(Diagnostic::new(
                    "L002",
                    format!("identifiers are lowercase snake_case, found `{word}`"),
                    Span::new(start, i),
                ));
            }
            if RESERVED.contains(&word) {
                return Err(Diagnostic::new(
                    "L005",
                    format!("`{word}` is reserved for a future version of spock"),
                    Span::new(start, i),
                ));
            }
            let kind = keyword(word).unwrap_or_else(|| TokenKind::Ident(word.to_string()));
            tokens.push(Token {
                kind,
                span: Span::new(start, i),
            });
            continue;
        }

        // uppercase start: still an identifier lexically, but rejected (L002)
        if b.is_ascii_uppercase() {
            i += 1;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            let word = &source[start..i];
            return Err(Diagnostic::new(
                "L002",
                format!("identifiers are lowercase snake_case, found `{word}`"),
                Span::new(start, i),
            ));
        }

        let ch = source[start..].chars().next().expect("in-bounds char");
        return Err(Diagnostic::new(
            "L001",
            format!("unexpected character `{ch}`"),
            Span::new(start, start + ch.len_utf8()),
        ));
    }

    tokens.push(Token {
        kind: TokenKind::Eof,
        span: Span::new(source.len(), source.len()),
    });
    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(source: &str) -> Vec<TokenKind> {
        lex(source).unwrap().into_iter().map(|t| t.kind).collect()
    }

    #[test]
    fn lexes_table_declaration() {
        assert_eq!(
            kinds("table user { key id: uuid = auto }"),
            vec![
                TokenKind::KwTable,
                TokenKind::Ident("user".into()),
                TokenKind::LBrace,
                TokenKind::KwKey,
                TokenKind::Ident("id".into()),
                TokenKind::Colon,
                TokenKind::KwUuid,
                TokenKind::Eq,
                TokenKind::KwAuto,
                TokenKind::RBrace,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn lexes_literals_and_comments() {
        assert_eq!(
            kinds("// hi\n\"a \\\"b\\\"\" -42 true"),
            vec![
                TokenKind::Str("a \"b\"".into()),
                TokenKind::Int(-42),
                TokenKind::KwTrue,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn rejects_uppercase_identifier() {
        assert_eq!(lex("table User {}").unwrap_err().code, "L002");
        assert_eq!(lex("table uSer {}").unwrap_err().code, "L002");
    }

    #[test]
    fn rejects_reserved_keyword() {
        assert_eq!(lex("view feed {}").unwrap_err().code, "L005");
    }

    #[test]
    fn rejects_unterminated_string() {
        assert_eq!(lex("\"abc").unwrap_err().code, "L003");
    }

    #[test]
    fn rejects_unknown_character() {
        assert_eq!(lex("table a { b: text; }").unwrap_err().code, "L001");
    }
}
