use proc_macro2::TokenStream;
use syn::{
    parse::{Parse, ParseStream}, parse_quote, visit_mut::{self, VisitMut}, Attribute, Pat, Result, Token
};
use quote::{quote, ToTokens, TokenStreamExt};
use crate::tools;

pub fn impl_event_subscriber(mut ast: syn::ItemImpl) -> TokenStream {
    let mut visitor = SubscriberVisitor::default();
    visitor.visit_item_impl_mut(&mut ast);
    let event_loop = visitor.event_loop;
    println!("{}", stringify(&event_loop));
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
    _colon_token: Token![:],
    query_type: syn::Type,
    _fat_arrow_token: Token![=>],
    pattern: EventLoopPattern
}

struct EventLoopPattern {
    query_key: syn::Member,
    pattern: syn::Pat
}

impl Parse for EventLoopPattern {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let mut pattern = syn::Pat::parse_single(input)?;
        let mut visitor = PatternVisitor::default();

        visitor.visit_pat_mut(&mut pattern);
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

#[derive(Default)]
struct EventLoop {
    queues: Vec<SubscribeQueue>
}

impl ToTokens for EventLoop {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let command_items: Vec<syn::Ident> = self.queues
            .iter()
            .map(|queue| tools::enum_case(&queue.name))
            .collect();
        
        tokens.append_all(quote! {
            enum Command {
                #(#command_items),*
            }
        });
    }
}

// This should be used as a field inside another structure
// because we need to output all macro occurances us our domain-specific structs
#[derive(Default)]
struct SubscriberVisitor {
    current_fn: Option<syn::Ident>,
    event_loop: EventLoop
}

impl VisitMut for SubscriberVisitor {
    fn visit_impl_item_fn_mut(&mut self, node: &mut syn::ImplItemFn) {
        self.current_fn = Some(node.sig.ident.clone());
        visit_mut::visit_impl_item_fn_mut(self, node);
    }
    
    fn visit_expr_mut(&mut self, node: &mut syn::Expr) {
        if let syn::Expr::Macro(expr) = node {
            if expr.mac.path.is_ident("subscribe") {
                let invocation: SubscribeInvocation = expr.mac
                    .parse_body()
                    .unwrap();
                let context = self.current_fn.as_ref().unwrap().clone();
                let queue = SubscribeQueue::new(context, invocation);
                *expr = parse_quote!(#queue);
                self.event_loop.queues.push(queue);
            }
        }
        visit_mut::visit_expr_mut(self, node);
    }
}
