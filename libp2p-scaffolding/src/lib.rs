use proc_macro::TokenStream;

mod event_subscriber;
mod tools;

#[proc_macro]
pub fn event_subscriber(input: TokenStream) -> TokenStream {
    let ast = syn::parse_macro_input!(input as syn::ItemImpl);
    event_subscriber::impl_event_subscriber(ast).into()
}
