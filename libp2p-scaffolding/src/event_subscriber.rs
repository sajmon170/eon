use proc_macro2::TokenStream;
use syn::{
    parse::{Parse, ParseStream}, parse_quote, visit_mut::{self, VisitMut}, Attribute, Pat, Result, Token
};
use quote::{quote, ToTokens, TokenStreamExt};
use crate::tools;

pub fn impl_event_subscriber(mut ast: syn::ItemImpl) -> TokenStream {
    /*
    let mut command_items = Vec::new();
    
    while let Some(syn::ImplItem::Fn(func)) = ast.items.iter().next() {
        parse_fn(func);
        command_items.push(tools::enum_case(&func.sig.ident));
    }
    
    quote! {
        enum Command {
            #(#command_items),*
        }
    }
    */
    let mut visitor = SubscriberVisitor::default();
    visitor.visit_item_impl_mut(&mut ast);
    quote!(#ast)
}

// Corresponds to a single function that calls the subscribe! macro
struct SubscribeQueue {
    name: syn::Ident,
    invocation: SubscribeInvocation
}

impl SubscribeQueue {
    fn new(name: syn::Ident, invocation: SubscribeInvocation) -> Self {
        Self { name, invocation }
    }
}

fn stringify<T: ToTokens>(node: &T) -> String {
    quote!(#node).to_string()
}

impl ToTokens for SubscribeQueue {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let name = stringify(&self.name);
        let query_id = stringify(&self.invocation.query_id);
        let query_type = stringify(&self.invocation.query_type);
        let query_key = stringify(&self.invocation.pattern.query_key);
        let pattern = stringify(&self.invocation.pattern.pattern);
        tokens.append_all(quote! {
            println!("Name: {:?}, Query id: {:?}, Query type: {:?}, Key: {:?}, pattern: {:?}", #name, #query_id, #query_type, #query_key, #pattern)
        });
    }
}

// Corresponds to the subscribe! pseudomacro itself
#[derive(syn_derive::Parse)]
struct SubscribeInvocation {
    query_id: syn::Ident,
    colon_token: Token![:],
    query_type: syn::Type,
    fat_arrow_token: Token![=>],
    pattern: EventLoopPattern
}

struct EventLoopPattern {
    query_key: syn::Member,
    pattern: syn::PatStruct
}

impl Parse for EventLoopPattern {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let parsed = syn::Pat::parse_single(input)?;
        let syn::Pat::Struct(mut pattern) = parsed else {
            return Err(input.error("Not a struct pattern"));
        };
        
        let mut visitor = PatternVisitor::default();

        visitor.visit_pat_struct_mut(&mut pattern);
        let key_no = visitor.keys.len();
        if key_no != 1 {
            return Err(input.error(format!(
                "The subscribe! macro requires 1 key, but {key_no} were provided"
            )));
        }

        let query_key = visitor.keys[0].member.clone();
        Ok(Self {
            query_key,
            pattern
        })
    }
}

#[derive(Default)]
struct PatternVisitor {
    keys: Vec<syn::FieldPat>
}

impl VisitMut for PatternVisitor {
    fn visit_field_pat_mut(&mut self, node: &mut syn::FieldPat) {
        let attrs: Vec<Attribute> = node.attrs
            .extract_if(.., |attr| attr.path().is_ident("key"))
            .collect();

        if !attrs.is_empty() {
            self.keys.push(node.clone());
        }
        
        visit_mut::visit_field_pat_mut(self, node);
    }
}

struct EventLoop {
    queues: Vec<SubscribeQueue> 
}

// This should be used as a field inside another structure
// because we need to output all macro occurances us our domain-specific structs
#[derive(Default)]
struct SubscriberVisitor {
    current_fn: Option<syn::Ident>
}

impl VisitMut for SubscriberVisitor {
    fn visit_impl_item_fn_mut(&mut self, node: &mut syn::ImplItemFn) {
        self.current_fn = Some(node.sig.ident.clone());
        visit_mut::visit_impl_item_fn_mut(self, node);
    }
    
    fn visit_expr_mut(&mut self, node: &mut syn::Expr) {
        if let syn::Expr::Macro(ref mut expr) = node {
            if expr.mac.path.is_ident("subscribe") {
                let invocation: SubscribeInvocation = expr.mac
                    .parse_body()
                    .unwrap();
                let context = self.current_fn.as_ref().unwrap().clone();
                let queue = SubscribeQueue::new(context, invocation);
                *expr = parse_quote!(#queue);
            }
        }
        visit_mut::visit_expr_mut(self, node);
    }
}
