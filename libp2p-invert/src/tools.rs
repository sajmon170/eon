use syn::Ident;
use proc_macro2::Span;

pub fn enum_case(ident: &Ident) -> Ident {
    let transformed = format!("{}", heck::AsUpperCamelCase(ident.to_string()));
    Ident::new(&transformed, Span::call_site())
}
