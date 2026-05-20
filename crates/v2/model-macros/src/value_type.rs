//! Support to parse a syn::Type into an IR type.
//!
//! This is necessary to be able to build the IR in the entity derive macro for
//! validation.
//!
//! TODO(johanpel): Proc macros run before name resolution so if a user does
//! something like use x::String; #[derive(entity)] struct Foo { bar: String },
//! then when we parse String, we just see those tokens, but not the use
//! declaration. So it should have been parsed into ValueType::Attributes, but
//! instead it will parse as ValueType::String.
//!
use quent_v2_model_ir::{
    attributes::{EntityRefKind, EntityRefTarget},
    identifier::Identifier,
    qualifications::{QualificationKind, QualificationRefKind, resource_group::RgRefKind},
    value_type::ValueType,
};

pub fn parse_value_type(ty: &syn::Type) -> syn::Result<ValueType> {
    let syn::Type::Path(type_path) = ty else {
        return Err(syn::Error::new_spanned(
            ty,
            "unsupported type: entity payload fields must be a named path type",
        ));
    };

    let last = type_path
        .path
        .segments
        .last()
        .ok_or_else(|| syn::Error::new_spanned(ty, "type has no path segments"))?;
    let name = last.ident.to_string();

    // Primitive types
    let atom = match name.as_str() {
        "bool" => Some(ValueType::Bool),
        "u8" => Some(ValueType::U8),
        "u16" => Some(ValueType::U16),
        "u32" => Some(ValueType::U32),
        "u64" => Some(ValueType::U64),
        "i8" => Some(ValueType::I8),
        "i16" => Some(ValueType::I16),
        "i32" => Some(ValueType::I32),
        "i64" => Some(ValueType::I64),
        "f32" => Some(ValueType::F32),
        "f64" => Some(ValueType::F64),
        "String" => Some(ValueType::String),
        "Uuid" => Some(ValueType::Uuid),
        "CustomAttributes" => Some(ValueType::CustomAttributes),
        _ => None,
    };
    if let Some(v) = atom {
        if !last.arguments.is_empty() {
            return Err(syn::Error::new_spanned(
                &last.arguments,
                format!("`{name}` does not take type arguments"),
            ));
        }
        return Ok(v);
    }

    // Composite types
    match name.as_str() {
        "Option" => {
            let inner = parse_generic(&last.arguments, ty)?;
            Ok(ValueType::Option(Box::new(parse_value_type(inner)?)))
        }
        "Vec" => {
            let inner = parse_generic(&last.arguments, ty)?;
            Ok(ValueType::List(Box::new(parse_value_type(inner)?)))
        }
        "EntityRef" => {
            let syn::PathArguments::AngleBracketed(args) = &last.arguments else {
                return Ok(ValueType::Attributes(Identifier::new_unchecked(
                    name.as_str(),
                )));
            };
            let mut type_args = args.args.iter().filter_map(|a| match a {
                syn::GenericArgument::Type(t) => Some(t),
                _ => None,
            });
            let entity_type = type_args
                .next()
                .ok_or_else(|| syn::Error::new_spanned(args, "EntityRef target type missing"))
                .and_then(parse_entity_ref_target)?;
            let role_type = match type_args.next() {
                Some(t) => parse_entity_ref_kind(t)?,
                None => EntityRefKind::Plain,
            };
            Ok(ValueType::EntityRef {
                entity_type,
                role_type,
            })
        }
        "Usage" => {
            todo!()
            // let _inner = single_generic_arg(&last.arguments, ty)?;
            // Ok(ValueType::Attributes(name))
        }
        // Assume a user-defined Attributes-derived type for anything else.
        _ => Ok(ValueType::Attributes(Identifier::new_unchecked(name))),
    }
}

fn parse_entity_ref_target(ty: &syn::Type) -> syn::Result<EntityRefTarget> {
    let syn::Type::Path(type_path) = ty else {
        return Err(syn::Error::new_spanned(
            ty,
            "EntityRef target must be a named path type",
        ));
    };
    let last = type_path
        .path
        .segments
        .last()
        .ok_or_else(|| syn::Error::new_spanned(ty, "EntityRef target has no path segments"))?;
    if !last.arguments.is_empty() {
        return Err(syn::Error::new_spanned(
            &last.arguments,
            "EntityRef target type does not take type arguments",
        ));
    }
    let name = last.ident.to_string();
    Ok(match name.as_str() {
        "AnyEntity" => EntityRefTarget::Any,
        "AnyRg" => EntityRefTarget::AnyQualified(QualificationKind::ResourceGroup),
        _ => EntityRefTarget::Specific(Identifier::new_unchecked(name)),
    })
}

fn parse_entity_ref_kind(ty: &syn::Type) -> syn::Result<EntityRefKind> {
    let syn::Type::Path(type_path) = ty else {
        return Err(syn::Error::new_spanned(
            ty,
            "EntityRef role must be a named path type",
        ));
    };
    let last = type_path
        .path
        .segments
        .last()
        .ok_or_else(|| syn::Error::new_spanned(ty, "EntityRef role has no path segments"))?;
    if !last.arguments.is_empty() {
        return Err(syn::Error::new_spanned(
            &last.arguments,
            "EntityRef role type does not take type arguments",
        ));
    }
    let name = last.ident.to_string();
    match name.as_str() {
        "PlainRef" => Ok(EntityRefKind::Plain),
        "RgParentRef" => Ok(EntityRefKind::Qualification(
            QualificationRefKind::ResourceGroup(RgRefKind::Parent),
        )),
        // Future ref kinds for Resource and others land here.
        _ => Err(syn::Error::new_spanned(
            ty,
            format!("unknown EntityRef role type: `{name}`"),
        )),
    }
}

fn parse_generic<'a>(
    args: &'a syn::PathArguments,
    span_source: &syn::Type,
) -> syn::Result<&'a syn::Type> {
    let syn::PathArguments::AngleBracketed(angled) = args else {
        return Err(syn::Error::new_spanned(
            span_source,
            "expected one generic argument",
        ));
    };
    if angled.args.len() != 1 {
        return Err(syn::Error::new_spanned(
            angled,
            "expected exactly one type argument",
        ));
    }
    let syn::GenericArgument::Type(t) = angled.args.first().unwrap() else {
        return Err(syn::Error::new_spanned(angled, "expected a type argument"));
    };
    Ok(t)
}
