#![allow(dead_code, unused)]
use proc_macro::TokenStream;

mod event_subscriber;
mod swarm_client;
mod tools;

#[proc_macro_attribute]
pub fn event_subscriber(attr: TokenStream, input: TokenStream) -> TokenStream {
    let name = syn::parse_macro_input!(attr as syn::Ident);
    let ast = syn::parse_macro_input!(input as syn::ItemImpl);
    let output: TokenStream = event_subscriber::impl_event_subscriber(ast, name).into();

    // let out = prettyplease::unparse(&syn::parse_file(&output.to_string()).unwrap());
    // println!("{out}");

    output
}

#[proc_macro_attribute]
pub fn swarm_client(attr: TokenStream, input: TokenStream) -> TokenStream {
    let name = syn::parse_macro_input!(attr as syn::Ident);
    let ast = syn::parse_macro_input!(input as syn::ItemStruct);
    let output: TokenStream = swarm_client::impl_swarm_client(ast, name).into();

    // let out = prettyplease::unparse(&syn::parse_file(&output.to_string()).unwrap());
    // println!("{out}");

    output
}
