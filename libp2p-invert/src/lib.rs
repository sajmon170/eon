use proc_macro::TokenStream;

mod event_subscriber;
mod tools;

#[proc_macro_attribute]
pub fn event_subscriber(attr: TokenStream, input: TokenStream) -> TokenStream {
    let name = syn::parse_macro_input!(attr as syn::Ident);
    let ast = syn::parse_macro_input!(input as syn::ItemImpl);
    event_subscriber::impl_event_subscriber(ast, name).into()
}
