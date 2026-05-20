use quent_v2_model_ir::qualifications::fsm::{Fsm, State, Transition};
use syn::{Token, parse::Parse};

pub fn parse(content: &syn::parse::ParseBuffer) -> syn::Result<Fsm> {
    let pairs = content.parse_terminated(TransitionPair::parse, syn::Token![,])?;
    let transitions = pairs
        .iter()
        .map(|p| {
            Ok(Transition {
                source: ident_to_state(&p.from)?,
                target: ident_to_state(&p.to)?,
            })
        })
        .collect::<syn::Result<Vec<_>>>()?;
    if transitions.is_empty() {
        return Err(syn::Error::new(
            content.span(),
            "fsm requires at least one transition",
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
