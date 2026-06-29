//! chumsky parser for the minimal QDL grammar (records and entities only).
//!
//! Uses the [`chumsky::error::Cheap`] error type: it carries no references into
//! the source, so the parser is covariant over the source lifetime and can be
//! returned as `impl Parser<'a, ...>`. Semantic checks (valid generics,
//! identifier grammar) happen during lowering, where they get precise messages.

use chumsky::error::Cheap;
use chumsky::prelude::*;

use crate::ast::{Ast, Cardinality, EntityDef, EventDef, FieldDef, Item, RecordDef, Ty};

/// Parser for a whole QDL file.
// Nested tuple annotations are required for chumsky's type inference.
#[allow(clippy::type_complexity)]
pub fn parser<'a>() -> impl Parser<'a, &'a str, Ast, extra::Err<Cheap>> {
    // Whitespace and non-doc (`//`) line comments. A `///` doc comment is left
    // for `docs` to capture, because the char after `//` must not be `/` here.
    let line_comment = just("//")
        .then(none_of("/"))
        .then(none_of("\n").repeated())
        .ignored();
    let ws = choice((one_of(" \t\r\n").ignored(), line_comment))
        .repeated()
        .ignored();

    // One or more consecutive `///` lines, joined and trimmed.
    let docs = just("///")
        .ignore_then(none_of("\n").repeated().collect::<String>())
        .then_ignore(ws)
        .repeated()
        .at_least(1)
        .collect::<Vec<String>>()
        .map(|lines| {
            lines
                .iter()
                .map(|l| l.trim())
                .collect::<Vec<_>>()
                .join("\n")
        });

    let ident = text::ascii::ident().padded_by(ws);

    // One generic level (e.g. `Vec<Stage>`, `Option<Plan>`). Deeper nesting is
    // not part of this minimal subset.
    let generic_arg = ident.delimited_by(just('<').padded_by(ws), just('>').padded_by(ws));
    let ty = ident
        .then(generic_arg.or_not())
        .map(|(head, arg): (&str, Option<&str>)| match arg {
            Some(a) => Ty::App(head.to_string(), Box::new(Ty::Named(a.to_string()))),
            None => Ty::Named(head.to_string()),
        });

    let field = docs
        .or_not()
        .then(ident)
        .then_ignore(just(':').padded_by(ws))
        .then(ty)
        .map(|((d, name), ty): ((Option<String>, &str), Ty)| FieldDef {
            docs: d,
            name: name.to_string(),
            ty,
        });

    let fields = field
        .separated_by(just(',').padded_by(ws))
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(just('{').padded_by(ws), just('}').padded_by(ws));

    let cardinality = choice((
        text::ascii::keyword("once").to(Cardinality::Once),
        text::ascii::keyword("multi").to(Cardinality::Multi),
    ))
    .padded_by(ws);

    let event = docs
        .or_not()
        .then(cardinality)
        .then(ident)
        .then_ignore(just(':').padded_by(ws))
        .then(fields)
        .map(
            |(((d, card), name), payload): (
                ((Option<String>, Cardinality), &str),
                Vec<FieldDef>,
            )| EventDef {
                docs: d,
                cardinality: card,
                name: name.to_string(),
                payload,
            },
        );

    let events = event
        .separated_by(just(',').padded_by(ws))
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(just('{').padded_by(ws), just('}').padded_by(ws));

    let record_body = text::ascii::keyword("record")
        .padded_by(ws)
        .ignore_then(ident)
        .then(fields)
        .map(|(name, fields): (&str, Vec<FieldDef>)| {
            Item::Record(RecordDef {
                docs: None,
                name: name.to_string(),
                fields,
            })
        });

    let entity_body = text::ascii::keyword("entity")
        .padded_by(ws)
        .ignore_then(ident)
        .then(events)
        .map(|(name, events): (&str, Vec<EventDef>)| {
            Item::Entity(EntityDef {
                docs: None,
                name: name.to_string(),
                events,
            })
        });

    let item = docs
        .or_not()
        .then(choice((record_body, entity_body)))
        .map(|(d, item)| match item {
            Item::Record(mut r) => {
                r.docs = d;
                Item::Record(r)
            }
            Item::Entity(mut e) => {
                e.docs = d;
                Item::Entity(e)
            }
        });

    let model = text::ascii::keyword("model")
        .padded_by(ws)
        .ignore_then(ident)
        .then_ignore(just(';').padded_by(ws))
        .map(|s: &str| s.to_string());

    model
        .then(item.repeated().collect::<Vec<_>>())
        .map(|(model, items)| Ast { model, items })
        .padded_by(ws)
        .then_ignore(end())
}
