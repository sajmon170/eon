use proc_macro2::TokenStream;
use syn::{
    parse::{Parse, ParseStream, Parser}, parse_quote, visit_mut::{self, VisitMut}, Attribute, Pat, Result, Token
};
use quote::{quote, ToTokens, TokenStreamExt};
use crate::tools;

pub fn impl_swarm_client(mut ast: syn::ItemStruct, name: syn::Ident) -> TokenStream {
    if let syn::Fields::Unit = ast.fields {
        ast.fields = syn::Fields::Named(parse_quote!({ }));
    }
    
    if let syn::Fields::Named(ref mut fields) = ast.fields {
        fields.named.push(
            syn::Field::parse_named
                .parse2(quote! { fn_sender: tokio::sync::mpsc::Sender<EventLoopFn> })
                .unwrap()
        );
        fields.named.push(
            syn::Field::parse_named
                .parse2(quote! { cancel: tokio_util::sync::CancellationToken })
                .unwrap()
        );
    }

    quote! {
        pub type EventLoopFn = Box<dyn FnOnce(&mut EventLoop) + Send + Sync>;
        #ast
    }
}
