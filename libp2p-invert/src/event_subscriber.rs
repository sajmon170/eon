use proc_macro2::TokenStream;
use syn::{
    parse::{Parse, ParseStream}, parse_quote, visit_mut::{self, VisitMut}, Attribute, Pat, Result, Token
};
use quote::{quote, ToTokens, TokenStreamExt};
use crate::tools;

pub fn impl_event_subscriber(mut ast: syn::ItemImpl, name: syn::Ident) -> TokenStream {
    let mut visitor = SubscriberVisitor::default();
    visitor.visit_item_impl_mut(&mut ast);
    let event_loop = visitor.event_loop;

    let self_ty = ast.self_ty.clone();
    quote!(
        #ast

        #event_loop

        pub type EventLoopFn = Box<dyn FnOnce(&mut EventLoop) + Send + Sync>;

        impl #self_ty {
            async fn register<T: Send + Sync + 'static>(
                &mut self,
                func: impl FnOnce(&mut libp2p::swarm::Swarm<Behaviour>) -> T + Send + Sync + 'static
            ) -> Result<T, Canceled> {
                let (tx, rx) = futures::channel::oneshot::channel();
                self.fn_sender.send(Box::new(move |event_loop| {
                    let output = func(&mut event_loop.swarm);
                    let _ = tx.send(output);
                })).await;

                rx.await
            }

            async fn add_pending<T: Send + Sync + 'static>(
                &mut self,
                func: impl FnOnce(&mut PendingQueries, futures::channel::oneshot::Sender<T>) + Send + Sync + 'static
            ) -> futures::channel::oneshot::Receiver<T> {
                let (tx, rx) = futures::channel::oneshot::channel();
                self.fn_sender.send(Box::new(move |event_loop| {
                    func(&mut event_loop.pending, tx);
                })).await;

                rx
            }
        }
    )
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
        let name = tools::member_case(&self.name);
        let query_id = &self.invocation.query_id;

        tokens.append_all(quote! {
            async {
                let rx = self.add_pending(move |db, sender| {
                    db.#name.insert(#query_id, sender);
                }).await;

                rx.await
            }
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

impl EventLoop {
    fn append_pending_queries_struct(&self, tokens: &mut TokenStream) {
        let db_items = self.queues
            .iter()
            .map(|queue| tools::member_case(&queue.name));

        let db_types = self.queues
            .iter()
            .map(|queue| &queue.invocation.query_type);
        
        // TODO - make the behaviour generic
        tokens.append_all(quote! {
            #[derive(Default)]
            pub(crate) struct PendingQueries {
                #(pub #db_items: HashMap<#db_types, futures::channel::oneshot::Sender<libp2p::swarm::SwarmEvent<BehaviourEvent>>>),*
            }
        });
    }
    
    fn append_event_loop_struct(&self, tokens: &mut TokenStream) {
        tokens.append_all(quote! {
            pub(crate) struct EventLoop {
                pub swarm: libp2p::swarm::Swarm<Behaviour>,
                pub pending: PendingQueries,
                fn_receiver: futures::channel::mpsc::Receiver<EventLoopFn>,
            }
        })
    }

    fn append_event_loop_impl(&self, tokens: &mut TokenStream) {
        let names = self.queues
            .iter()
            .map(|queue| tools::member_case(&queue.name));

        let patterns = self.queues
            .iter()
            .map(|queue| &queue.invocation.pattern.pattern);
        
        let keys = self.queues
            .iter()
            .map(|queue| &queue.invocation.pattern.query_key);
        
        tokens.append_all(quote! {
            impl EventLoop {
                pub fn new(
                    swarm: libp2p::swarm::Swarm<Behaviour>,
                    fn_receiver: mpsc::Receiver<EventLoopFn>
                ) -> Self {
                    Self {
                        swarm,
                        fn_receiver,
                        pending: Default::default()
                    }
                }

                pub(crate) async fn run(mut self) {
                    loop {
                        futures::select! {
                            event = self.swarm.select_next_some() => self.handle_event(event).await,
                            func = self.fn_receiver.select_next_some() => func(&mut self),
                        }
                    }
                }

                async fn handle_event(&mut self, event: libp2p::swarm::SwarmEvent<BehaviourEvent>) {
                    match &event {
                        #(#patterns => {
                            let sender = self
                                .pending
                                .#names
                                .remove(&#keys)
                                .unwrap();
                            
                            let _ = sender.send(event);
                        })*
                        _ => {}
                    }
                }
            }
        });
    }
}

impl ToTokens for EventLoop {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.append_pending_queries_struct(tokens);
        self.append_event_loop_struct(tokens);
        self.append_event_loop_impl(tokens);
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
                *node = parse_quote!(#queue);
                self.event_loop.queues.push(queue);
            }
        }
        visit_mut::visit_expr_mut(self, node);
    }
}
