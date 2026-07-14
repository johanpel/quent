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
//! `T?` is sugar for `Option<T>`. `Ref` takes no parameter, leaving the spot
//! free for later syntax extensions. A bare name is a record reference,
//! checked against the declared records by base constraint validation.

use quent_schema::{Annotations, DataType, Identifier};

/// Maximum type-expression nesting, matching `quent-instrumentation-build`.
const MAX_DEPTH: usize = 64;

/// Names with a fixed meaning in type expressions, so a record or entity may
/// not be declared under one.
pub(crate) const RESERVED_TYPE_NAMES: &[&str] = &[
    "Dynamic", "Option", "Ref", "String", "Uuid", "Vec", "bool", "f32", "f64", "i16", "i32", "i64",
    "i8", "u16", "u32", "u64", "u8",
];

/// Parse a type expression, returning the type or a human-readable reason.
pub(crate) fn parse_type(text: &str) -> Result<DataType, String> {
    let mut lexer = Lexer::new(text);
    let ty = parse(&mut lexer, 0)?;
    match lexer.next() {
        Token::End => Ok(ty),
        tok => Err(format!("unexpected trailing {}", tok.describe())),
    }
}

fn parse(lexer: &mut Lexer, depth: usize) -> Result<DataType, String> {
    if depth > MAX_DEPTH {
        return Err(format!("nests deeper than {MAX_DEPTH} wrappers"));
    }
    let mut ty = parse_atom(lexer, depth)?;
    while lexer.eat(&Token::Question) {
        ty = DataType::Option(Box::new(ty));
    }
    Ok(ty)
}

fn parse_atom(lexer: &mut Lexer, depth: usize) -> Result<DataType, String> {
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
            let inner = parse(lexer, depth + 1)?;
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
                     `{ ref: , data: <type> }` form"
                        .to_string(),
                );
            }
            DataType::EntityRef {
                data: None,
                annotations: Annotations::default(),
            }
        }
        _ => DataType::Record(
            Identifier::try_new(&name).expect("the lexer only produces valid identifiers"),
        ),
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

    #[test]
    fn parses_scalars_and_compositions() {
        assert_eq!(parse_type("bool").unwrap(), DataType::Bool);
        assert_eq!(
            parse_type("Vec<u32>").unwrap(),
            DataType::List(Box::new(DataType::U32))
        );
        assert_eq!(
            parse_type("String?").unwrap(),
            parse_type("Option<String>").unwrap()
        );
        assert_eq!(
            parse_type("Vec<Stage>").unwrap(),
            DataType::List(Box::new(DataType::Record(
                Identifier::try_new("Stage").unwrap()
            )))
        );
    }

    #[test]
    fn parses_bare_ref() {
        assert!(matches!(
            parse_type("Ref").unwrap(),
            DataType::EntityRef { data: None, .. }
        ));
    }

    #[test]
    fn rejects_malformed() {
        for (expr, needle) in [
            ("", "expected a type"),
            ("Vec", "needs a type argument"),
            ("Vec<u8", "missing `>`"),
            ("&Engine", "unexpected character `&`"),
            ("Ref<Engine>", "`Ref` takes no type parameter"),
        ] {
            assert!(parse_type(expr).unwrap_err().contains(needle), "{expr:?}");
        }
    }
}
