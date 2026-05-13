#![warn(clippy::pedantic)]

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{DeriveInput, Expr, ExprArray, ExprLit, Lit, Meta, parse_macro_input};

/// Derives `Format` for a type, reading extensions from `#[format(extensions = [...])]`.
///
/// # Example
/// ```ignore
/// #[derive(Format)]
/// #[format(extensions = ["osu"])]
/// pub struct OsuDecoder;
/// ```
#[proc_macro_derive(Format, attributes(format))]
pub fn derive_format(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match impl_format(&input) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

fn impl_format(input: &DeriveInput) -> syn::Result<TokenStream2> {
    let name = &input.ident;
    let extensions = parse_extensions(input)?;
    Ok(quote! {
        impl ::rox::codec::Format for #name {
            const EXTENSIONS: &'static [&'static str] = &[#(#extensions),*];
        }
    })
}

fn parse_extensions(input: &DeriveInput) -> syn::Result<Vec<String>> {
    for attr in &input.attrs {
        if !attr.path().is_ident("format") {
            continue;
        }
        let meta = attr.parse_args::<Meta>()?;
        match meta {
            Meta::NameValue(nv) if nv.path.is_ident("extensions") => {
                return extract_string_array(&nv.value);
            }
            _ => {
                return Err(syn::Error::new_spanned(
                    attr,
                    "expected #[format(extensions = [\"ext1\", \"ext2\"])]",
                ));
            }
        }
    }
    Ok(Vec::new())
}

fn extract_string_array(expr: &Expr) -> syn::Result<Vec<String>> {
    let Expr::Array(ExprArray { elems, .. }) = expr else {
        return Err(syn::Error::new_spanned(
            expr,
            "expected array literal like [\"ext\"]",
        ));
    };
    elems
        .iter()
        .map(|e| {
            let Expr::Lit(ExprLit {
                lit: Lit::Str(s), ..
            }) = e
            else {
                return Err(syn::Error::new_spanned(e, "expected string literal"));
            };
            Ok(s.value())
        })
        .collect()
}
