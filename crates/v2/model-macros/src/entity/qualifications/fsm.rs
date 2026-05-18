use quent_v2_model_ir::qualifications::fsm::{Fsm, State, Transition};
use syn::{Token, parse::Parse};

pub fn parse(content: &syn::parse::ParseBuffer) -> syn::Result<Fsm> {
    let mut transitions: Vec<Transition> = Vec::new();

    while !content.is_empty() {
        let path: syn::Path = content.parse()?;

        if path.is_ident("transitions") {
            let inner;
            syn::parenthesized!(inner in content);
            let pairs = inner.parse_terminated(TransitionPair::parse, Token![,])?;
            for pair in pairs {
                transitions.push(Transition {
                    source: ident_to_state(&pair.from)?,
                    target: ident_to_state(&pair.to)?,
                });
            }
        } else {
            let key = path
                .get_ident()
                .map(|i| i.to_string())
                .unwrap_or_else(|| "<non-ident>".into());
            return Err(syn::Error::new_spanned(
                &path,
                format!("unknown fsm argument: {key}"),
            ));
        }

        if content.peek(Token![,]) {
            let _: Token![,] = content.parse()?;
        }
    }

    if transitions.is_empty() {
        return Err(syn::Error::new(
            content.span(),
            "fsm requires a transitions(...) declaration",
        ));
    }

    Ok(Fsm { transitions })
}

struct TransitionPair {
    from: syn::Ident,
    to: syn::Ident,
}

impl Parse for TransitionPair {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let from: syn::Ident = input.parse()?;
        let _arrow: Token![->] = input.parse()?;
        let to: syn::Ident = input.parse()?;
        Ok(Self { from, to })
    }
}

fn ident_to_state(ident: &syn::Ident) -> syn::Result<State> {
    State::try_from(ident.to_string().as_str())
        .map_err(|e| syn::Error::new(ident.span(), e.to_string()))
}
