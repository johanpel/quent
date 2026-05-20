pub fn ident(s: &str) -> Identifier {
    Identifier::new_unchecked(s)
}

#[macro_export]
macro_rules! rust_path {
    ($name:expr) => {
        format!("{}::{}", module_path!(), $name)
    };
}

use quent_v2_model_ir::identifier::Identifier;

pub use crate::rust_path;
