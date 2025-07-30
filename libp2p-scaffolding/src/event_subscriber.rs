use proc_macro2::TokenStream;
use quote::quote;
use crate::tools;

pub fn impl_event_subscriber(ast: syn::ItemImpl) -> TokenStream {
    let mut command_items = Vec::new();
    
    while let Some(syn::ImplItem::Fn(func)) = ast.items.iter().next() {
        parse_fn(func);
        command_items.push(tools::enum_case(&func.sig.ident));
    }
    
    quote! {
        enum Command {
            #(#command_items),*
        }
    }.into()
}

fn parse_fn(func: &syn::ImplItemFn) {
    for statement in &func.block.stmts {
    }
}
