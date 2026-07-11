//! Lexer (docs/spec/v0.md §2).

use crate::diag::Diagnostic;
use crate::span::Span;

#[derive(Clone, Debug, PartialEq)]
pub enum TokenKind {
    Ident(String),
    Str(String),
    Int(i64),
    Float(f64),

    // punctuation
    LBrace,
    RBrace,
    LParen,
    RParen,
    LBracket,
    RBracket,
    Colon,
    Comma,
    Question,
    Eq,
    Arrow,
    Bang,
    Pipe,

    // active keywords (§2.3)
    KwTable,
    KwRecord,
    KwFn,
    KwMut,
    KwKey,
    KwUnique,
    KwCheck,
    KwSeed,
    KwOn,
    KwDelete,
    KwRestrict,
    KwCascade,
    KwSet,
    KwNull,
    KwText,
    KwInt,
    KwFloat,
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
            TokenKind::Float(_) => "float literal".to_string(),
            TokenKind::LBrace => "`{`".to_string(),
            TokenKind::RBrace => "`}`".to_string(),
            TokenKind::LParen => "`(`".to_string(),
            TokenKind::RParen => "`)`".to_string(),
            TokenKind::LBracket => "`[`".to_string(),
            TokenKind::RBracket => "`]`".to_string(),
            TokenKind::Colon => "`:`".to_string(),
            TokenKind::Comma => "`,`".to_string(),
            TokenKind::Question => "`?`".to_string(),
            TokenKind::Eq => "`=`".to_string(),
            TokenKind::Arrow => "`->`".to_string(),
            TokenKind::Bang => "`!`".to_string(),
            TokenKind::Pipe => "`|`".to_string(),
            TokenKind::Eof => "end of file".to_string(),
            kw => format!("keyword `{}`", keyword_text(kw)),
        }
    }
}

fn keyword_text(kind: &TokenKind) -> &'static str {
    match kind {
        TokenKind::KwTable => "table",
        TokenKind::KwRecord => "record",
        TokenKind::KwFn => "fn",
        TokenKind::KwMut => "mut",
        TokenKind::KwKey => "key",
        TokenKind::KwUnique => "unique",
        TokenKind::KwCheck => "check",
        TokenKind::KwSeed => "seed",
        TokenKind::KwOn => "on",
        TokenKind::KwDelete => "delete",
        TokenKind::KwRestrict => "restrict",
        TokenKind::KwCascade => "cascade",
        TokenKind::KwSet => "set",
        TokenKind::KwNull => "null",
        TokenKind::KwText => "text",
        TokenKind::KwInt => "int",
        TokenKind::KwFloat => "float",
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
/// `unsafe` is reserved for the runtime-integrity tier (RFD 0011 §3) —
/// the verification-gap tier is the contextual `unchecked`.
const RESERVED: &[&str] = &[
    "view",
    "role",
    "policy",
    "error",
    "state",
    "extern",
    "unsafe",
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
        "record" => TokenKind::KwRecord,
        "fn" => TokenKind::KwFn,
        "mut" => TokenKind::KwMut,
        "key" => TokenKind::KwKey,
        "unique" => TokenKind::KwUnique,
        "check" => TokenKind::KwCheck,
        "seed" => TokenKind::KwSeed,
        "on" => TokenKind::KwOn,
        "delete" => TokenKind::KwDelete,
        "restrict" => TokenKind::KwRestrict,
        "cascade" => TokenKind::KwCascade,
        "set" => TokenKind::KwSet,
        "null" => TokenKind::KwNull,
        "text" => TokenKind::KwText,
        "int" => TokenKind::KwInt,
        "float" => TokenKind::KwFloat,
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
            b'[' => Some(TokenKind::LBracket),
            b']' => Some(TokenKind::RBracket),
            b':' => Some(TokenKind::Colon),
            b',' => Some(TokenKind::Comma),
            b'?' => Some(TokenKind::Question),
            b'=' => Some(TokenKind::Eq),
            b'!' => Some(TokenKind::Bang),
            b'|' => Some(TokenKind::Pipe),
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

        // triple-quoted raw string (§2.2): no escape processing, newlines
        // legal, terminates at the first `"""` — a body needing a literal
        // `"""` uses the single-line form
        if bytes[i..].starts_with(b"\"\"\"") {
            let content_start = i + 3;
            let mut j = content_start;
            loop {
                if j + 3 > bytes.len() {
                    return Err(Diagnostic::new(
                        "L006",
                        "unterminated triple-quoted string",
                        Span::new(start, bytes.len()),
                    ));
                }
                if &bytes[j..j + 3] == b"\"\"\"" {
                    break;
                }
                j += 1;
            }
            // `"` (0x22) never occurs inside a multi-byte UTF-8 sequence,
            // so both bounds land on character boundaries
            let value = source[content_start..j].to_string();
            i = j + 3;
            tokens.push(Token {
                kind: TokenKind::Str(value),
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

        // `->` (before the integer branch: `-` followed by `>` is never a
        // negative literal)
        if b == b'-' && bytes.get(i + 1) == Some(&b'>') {
            i += 2;
            tokens.push(Token {
                kind: TokenKind::Arrow,
                span: Span::new(start, i),
            });
            continue;
        }

        // numeric literal (minus only immediately before digits); a `.`
        // followed by a digit turns it into a float — plain decimal
        // notation only, no exponents in v0
        if b.is_ascii_digit() || (b == b'-' && bytes.get(i + 1).is_some_and(u8::is_ascii_digit)) {
            i += 1;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            let is_float = bytes.get(i) == Some(&b'.')
                && bytes.get(i + 1).is_some_and(u8::is_ascii_digit);
            if is_float {
                i += 1;
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                }
                let text = &source[start..i];
                // f64 parsing never fails on plain decimals — overflow
                // saturates to infinity — so the real guard is finiteness:
                // a non-finite value has no JSON spelling and would corrupt
                // the contract artifact
                let value: f64 = text.parse().unwrap_or(f64::INFINITY);
                if !value.is_finite() {
                    return Err(Diagnostic::new(
                        "L004",
                        format!("float literal `{text}` does not fit in a 64-bit float"),
                        Span::new(start, i),
                    ));
                }
                tokens.push(Token {
                    kind: TokenKind::Float(value),
                    span: Span::new(start, i),
                });
                continue;
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
    fn lexes_float_literals() {
        assert_eq!(
            kinds("0.5 -1.25"),
            vec![
                TokenKind::Float(0.5),
                TokenKind::Float(-1.25),
                TokenKind::Eof,
            ]
        );
        // a trailing dot is not a float: the int lexes, the `.` is stray
        assert_eq!(lex("1. x").unwrap_err().code, "L001");
        // `float` is an active keyword
        assert_eq!(kinds("float"), vec![TokenKind::KwFloat, TokenKind::Eof]);
        // overflow saturates in f64 parsing — the lexer must catch it
        let huge = format!("{}.0", "9".repeat(320));
        assert_eq!(lex(&huge).unwrap_err().code, "L004");
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
    fn fn_and_record_are_active_keywords() {
        assert_eq!(
            kinds("fn record mut"),
            vec![
                TokenKind::KwFn,
                TokenKind::KwRecord,
                TokenKind::KwMut,
                TokenKind::Eof
            ]
        );
        // `sql` is NOT a keyword — contextual in the parser only
        assert_eq!(
            kinds("sql"),
            vec![TokenKind::Ident("sql".into()), TokenKind::Eof]
        );
    }

    #[test]
    fn lexes_fn_punctuation() {
        assert_eq!(
            kinds("-> ! | [ ]"),
            vec![
                TokenKind::Arrow,
                TokenKind::Bang,
                TokenKind::Pipe,
                TokenKind::LBracket,
                TokenKind::RBracket,
                TokenKind::Eof,
            ]
        );
        // arrow between idents, and negative literals unbroken
        assert_eq!(
            kinds("x->y -42"),
            vec![
                TokenKind::Ident("x".into()),
                TokenKind::Arrow,
                TokenKind::Ident("y".into()),
                TokenKind::Int(-42),
                TokenKind::Eof,
            ]
        );
        // a bare `-` is still unexpected
        assert_eq!(lex("a - b").unwrap_err().code, "L001");
    }

    #[test]
    fn lexes_triple_quoted_strings() {
        // multi-line, raw: escapes are inert, quotes and `""` are content
        assert_eq!(
            kinds("\"\"\"\nSELECT 'a\"b' AS x, \"\"\n  FROM t\\n\n\"\"\""),
            vec![
                TokenKind::Str("\nSELECT 'a\"b' AS x, \"\"\n  FROM t\\n\n".into()),
                TokenKind::Eof,
            ]
        );
        // empty triple string
        assert_eq!(
            kinds("\"\"\"\"\"\""),
            vec![TokenKind::Str("".into()), TokenKind::Eof]
        );
        // the empty single-line string still lexes
        assert_eq!(
            kinds("\"\" x"),
            vec![
                TokenKind::Str("".into()),
                TokenKind::Ident("x".into()),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn rejects_unterminated_triple_string() {
        assert_eq!(lex("\"\"\"abc").unwrap_err().code, "L006");
        // four quotes: opener + a lone `"` of content, never terminated
        assert_eq!(lex("\"\"\"\"").unwrap_err().code, "L006");
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
