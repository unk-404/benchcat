//! Version dispatch. A compiler version never depends on a later version.

pub(crate) mod v1;
pub(crate) mod v2;

use proc_macro2::TokenStream;

pub(crate) fn compile(version: u32, text: &str) -> Result<TokenStream, String> {
    match version {
        1 => v1::compile(text),
        2 => v2::compile(text),
        other => Err(format!("unsupported .oops version {other}")),
    }
}
