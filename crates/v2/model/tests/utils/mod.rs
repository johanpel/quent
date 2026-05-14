#[macro_export]
macro_rules! rust_path {
    ($name:expr) => {
        format!("{}::{}", module_path!(), $name)
    };
}

pub use crate::rust_path;
