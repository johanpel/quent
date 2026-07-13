// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! The field type-expression mini-language.
//!
//! Grammar, with whitespace tolerated between tokens:
//!
//! ```text
//! type   := atom '?'*
//! atom   := 'Vec' '<' type '>' | 'Option' '<' type '>'
//!        |  'Ref'
//!        |  'bool' | 'Uuid' | 'String' | 'Dynamic'
//!        |  'u8'..'u64' | 'i8'..'i64' | 'f32' | 'f64'
//!        |  ident                                        # record reference
//! ident  := [A-Za-z][A-Za-z0-9_]*
//! ```
//!
//! `T?` is sugar for `Option<T>`; each `?` wraps once. `Ref` takes no
//! parameter, leaving the spot free for later syntax extensions. Record
//! references are collected for a deferred existence check once all
//! declarations are known.

use quent_schema::{Annotations, DataType, Identifier};
use saphyr_parser::Span;

use crate::diag::Sink;

// Mirrors quent-instrumentation-build's MAX_TYPE_DEPTH, so over-deep types
// fail here with a span instead of panicking later in codegen.
pub(crate) const MAX_TYPE_DEPTH: usize = 64;

/// Names with a fixed meaning in type expressions. Declaring a record or
/// entity under one of these would make it unreachable or confusing.
pub(crate) const RESERVED_TYPE_NAMES: &[&str] = &[
    "Dynamic", "Option", "Ref", "String", "Uuid", "Vec", "bool", "f32", "f64", "i16", "i32", "i64",
    "i8", "u16", "u32", "u64", "u8",
];

/// Names whose existence is checked after all declarations are known.
#[derive(Default)]
pub(crate) struct DeferredRefs {
    /// Record references: name, span, path.
    pub(crate) records: Vec<(String, Span, String)>,
}

/// Nesting depth of `Option`/`List`/`EntityRef` wrappers, counted as
/// `quent-instrumentation-build`'s type mapping does.
pub(crate) fn wrapper_depth(ty: &DataType) -> usize {
    match ty {
        DataType::Option(inner) | DataType::List(inner) => 1 + wrapper_depth(inner),
        DataType::EntityRef {
            data: Some(inner), ..
        } => 1 + wrapper_depth(inner),
        _ => 0,
    }
}

/// Parse one type expression. Errors go to `sink` at `span`.
pub(crate) fn parse_type_expr(
    text: &str,
    span: Span,
    path: &str,
    sink: &mut Sink,
    refs: &mut DeferredRefs,
) -> Option<DataType> {
    let mut lexer = Lexer::new(text);
    let detail = match parse_type(&mut lexer, span, path, refs, 0) {
        Ok(ty) => match lexer.next() {
            Token::End => return Some(ty),
            tok => format!("unexpected trailing {}", tok.describe()),
        },
        Err(detail) => detail,
    };
    sink.error(
        span,
        path,
        format!("invalid type expression `{text}`: {detail}"),
        None,
    );
    None
}

fn parse_type(
    lexer: &mut Lexer,
    span: Span,
    path: &str,
    refs: &mut DeferredRefs,
    depth: usize,
) -> Result<DataType, String> {
    // Bail while parsing, before recursion can overflow the stack; the
    // composed-type check in lowering handles depth across structured refs.
    if depth > MAX_TYPE_DEPTH {
        return Err(format!("nests deeper than {MAX_TYPE_DEPTH} wrappers"));
    }
    let mut ty = parse_atom(lexer, span, path, refs, depth)?;
    while lexer.eat(&Token::Question) {
        ty = DataType::Option(Box::new(ty));
    }
    Ok(ty)
}

fn parse_atom(
    lexer: &mut Lexer,
    span: Span,
    path: &str,
    refs: &mut DeferredRefs,
    depth: usize,
) -> Result<DataType, String> {
    let name = match lexer.next() {
        Token::Ident(name) => name,
        tok => return Err(format!("expected a type, found {}", tok.describe())),
    };
    let ty = match name.as_str() {
        "bool" => DataType::Bool,
        "Uuid" => DataType::Uuid,
        "String" => DataType::String,
        "u8" => DataType::U8,
        "u16" => DataType::U16,
        "u32" => DataType::U32,
        "u64" => DataType::U64,
        "i8" => DataType::I8,
        "i16" => DataType::I16,
        "i32" => DataType::I32,
        "i64" => DataType::I64,
        "f32" => DataType::F32,
        "f64" => DataType::F64,
        "Dynamic" => DataType::DynamicRecord,
        "Vec" | "Option" => {
            if !lexer.eat(&Token::Lt) {
                return Err(format!(
                    "`{name}` needs a type argument, e.g. `{name}<u32>`"
                ));
            }
            let inner = parse_type(lexer, span, path, refs, depth + 1)?;
            if !lexer.eat(&Token::Gt) {
                return Err(format!("missing `>` to close `{name}<`"));
            }
            match name.as_str() {
                "Vec" => DataType::List(Box::new(inner)),
                _ => DataType::Option(Box::new(inner)),
            }
        }
        "Ref" => {
            if lexer.eat(&Token::Lt) {
                return Err(
                    "`Ref` takes no type parameter; carried data uses the structured \
                     `{ ref: ..., data: <type> }` form"
                        .to_string(),
                );
            }
            DataType::EntityRef {
                data: None,
                annotations: Annotations::default(),
            }
        }
        _ => {
            let ident =
                Identifier::try_new(&name).expect("the lexer only produces valid identifiers");
            refs.records.push((name, span, path.to_string()));
            DataType::Record(ident)
        }
    };
    Ok(ty)
}

#[derive(Debug, PartialEq, Eq)]
enum Token {
    Ident(String),
    Lt,
    Gt,
    Question,
    End,
    Unexpected(char),
}

impl Token {
    fn describe(&self) -> String {
        match self {
            Token::Ident(name) => format!("`{name}`"),
            Token::Lt => "`<`".to_string(),
            Token::Gt => "`>`".to_string(),
            Token::Question => "`?`".to_string(),
            Token::End => "the end of the expression".to_string(),
            Token::Unexpected(c) => format!("unexpected character `{c}`"),
        }
    }
}

struct Lexer<'s> {
    chars: std::iter::Peekable<std::str::Chars<'s>>,
    peeked: Option<Token>,
}

impl<'s> Lexer<'s> {
    fn new(text: &'s str) -> Self {
        Self {
            chars: text.chars().peekable(),
            peeked: None,
        }
    }

    fn next(&mut self) -> Token {
        if let Some(tok) = self.peeked.take() {
            return tok;
        }
        while matches!(self.chars.peek(), Some(c) if c.is_whitespace()) {
            self.chars.next();
        }
        match self.chars.next() {
            None => Token::End,
            Some('<') => Token::Lt,
            Some('>') => Token::Gt,
            Some('?') => Token::Question,
            Some(c) if c.is_ascii_alphabetic() => {
                let mut name = String::from(c);
                while matches!(self.chars.peek(), Some(c) if c.is_ascii_alphanumeric() || *c == '_')
                {
                    name.push(self.chars.next().expect("peeked"));
                }
                Token::Ident(name)
            }
            Some(c) => Token::Unexpected(c),
        }
    }

    fn eat(&mut self, tok: &Token) -> bool {
        let next = self.next();
        if next == *tok {
            true
        } else {
            self.peeked = Some(next);
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Result<DataType, String> {
        let mut sink = Sink::new("<test>");
        let mut refs = DeferredRefs::default();
        match parse_type_expr(text, Span::default(), "t", &mut sink, &mut refs) {
            Some(ty) => Ok(ty),
            None => Err(sink.into_diagnostics().to_string()),
        }
    }

    #[test]
    fn parses_scalars_and_compositions() {
        assert_eq!(parse("bool").unwrap(), DataType::Bool);
        assert_eq!(parse("String").unwrap(), DataType::String);
        assert_eq!(
            parse("Vec<u32>").unwrap(),
            DataType::List(Box::new(DataType::U32))
        );
        assert_eq!(
            parse("Option<String>").unwrap(),
            DataType::Option(Box::new(DataType::String))
        );
        assert_eq!(parse("String?").unwrap(), parse("Option<String>").unwrap());
        assert_eq!(
            parse("u16??").unwrap(),
            DataType::Option(Box::new(DataType::Option(Box::new(DataType::U16))))
        );
        assert_eq!(
            parse(" Vec < String ? > ").unwrap(),
            DataType::List(Box::new(DataType::Option(Box::new(DataType::String))))
        );
    }

    #[test]
    fn parses_refs() {
        let DataType::EntityRef { data, annotations } = parse("Ref").unwrap() else {
            panic!("expected an entity ref");
        };
        assert!(data.is_none());
        assert_eq!(annotations, Annotations::default());
        assert_eq!(
            parse("Ref?").unwrap(),
            DataType::Option(Box::new(parse("Ref").unwrap()))
        );
    }

    #[test]
    fn rejects_malformed_expressions() {
        for (expr, detail) in [
            ("", "expected a type"),
            ("Vec", "needs a type argument"),
            ("Vec<u32", "missing `>`"),
            ("Vec<>", "expected a type"),
            ("u32?x", "unexpected trailing"),
            ("&Engine", "unexpected character `&`"),
            ("Vec<u32>>", "unexpected trailing"),
            ("Ref<Engine>", "`Ref` takes no type parameter"),
        ] {
            let err = parse(expr).unwrap_err();
            assert!(err.contains(detail), "{expr:?}: {err}");
        }
    }

    #[test]
    fn wrapper_depth_counts_all_wrappers() {
        let ty = parse("Vec<String?>?").unwrap();
        assert_eq!(wrapper_depth(&ty), 3);
        let ty = DataType::EntityRef {
            data: Some(Box::new(DataType::Option(Box::new(DataType::U8)))),
            annotations: Annotations::default(),
        };
        assert_eq!(wrapper_depth(&ty), 2);
    }
}
