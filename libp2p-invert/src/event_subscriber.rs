use proc_macro2::TokenStream;
use syn::{
    parse::{Parse, ParseStream}, parse_quote, visit_mut::{self, VisitMut}, Attribute, Pat, Result, Token
};
use quote::{format_ident, quote, ToTokens, TokenStreamExt};
use crate::tools;

pub fn impl_event_subscriber(mut ast: syn::ItemImpl, name: syn::Ident) -> TokenStream {
    let mut visitor = SubscriberVisitor::default();
    visitor.visit_item_impl_mut(&mut ast);
    let event_loop = visitor.event_loop;

    let self_ty = ast.self_ty.clone();
    quote!(
        #ast

        #event_loop

        // This should be implemented in the swarm_client macro instead
        // TODO: remove the keys hack
        impl #self_ty {
            async fn new(swarm: libp2p::swarm::Swarm<Behaviour>, keys: libp2p::identity::Keypair) -> Self {
                let (fn_sender, rx) = tokio::sync::mpsc::channel(1);
                let cancel = tokio_util::sync::CancellationToken::new();
                let _ = tokio::spawn(EventLoop::new(swarm, rx, cancel.clone()).run());

                Self {
                    fn_sender,
                    keys,
                    cancel
                }
            }
            
            async fn register<T: Send + Sync + 'static>(
                &self,
                func: impl FnOnce(&mut libp2p::swarm::Swarm<Behaviour>) -> T + Send + Sync + 'static
            ) -> Result<T, tokio::sync::oneshot::error::RecvError> {
                let (tx, rx) = tokio::sync::oneshot::channel();
                self.fn_sender.send(Box::new(move |event_loop| {
                    let output = func(&mut event_loop.swarm);
                    let _ = tx.send(output);
                })).await;

                rx.await
            }

            async fn add_pending<T: Send + Sync + 'static>(
                &self,
                func: impl FnOnce(&mut PendingQueries, tokio::sync::oneshot::Sender<T>) + Send + Sync + 'static
            ) -> tokio::sync::oneshot::Receiver<T> {
                let (tx, rx) = tokio::sync::oneshot::channel();
                self.fn_sender.send(Box::new(move |event_loop| {
                    func(&mut event_loop.pending, tx);
                })).await;

                rx
            }
        }

        impl std::ops::Drop for #self_ty {
            fn drop(&mut self) {
                self.cancel.cancel();
            }
        }
    )
}

// Corresponds to a single function that calls the subscribe! macro
#[derive(PartialEq, Eq)]
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
        let buffer = format_ident!("{name}_buffer");
        
        let output = match &self.invocation {
            SubscribeInvocation::WithKey(invocation) => {
                let query_id = &invocation.query_id;
                let pattern = &invocation.pattern.pattern;
                let output = &invocation.pattern.output;

                quote! {
                    async {
                        let rx = self.add_pending(move |db, sender| {
                            if let Some(out) = db.#buffer.remove(&#query_id) {
                                sender.send(out);
                            }
                            else {
                                db.#name.insert(#query_id, sender);
                            }
                        }).await;

                        rx.await.map(|out| {
                            match out {
                                #pattern => (#(#output),*),
                                _ => panic!()
                            }
                        })
                    }
                }
            }
            SubscribeInvocation::WithoutKey(invocation) => {
                let pattern = &invocation.pattern.pattern;
                let output = &invocation.pattern.output;

                quote! {
                    async {
                        let rx = self.add_pending(move |db, sender| {
                            if let Some(out) = db.#buffer.pop() {
                                sender.send(out);
                            }
                            else {
                                db.#name.insert(sender);
                            }
                        }).await;

                        rx.await.map(|out| {
                            match out {
                                #pattern => (#(#output),*),
                                _ => panic!()
                            }
                        })
                    }
                }
            }
        };

        tokens.append_all(output);
    }
}


#[derive(PartialEq, Eq)]
enum SubscribeInvocation {
    WithKey(SubscribeInvocationWithKey),
    WithoutKey(SubscribeInvocationWithoutKey)
}

impl Parse for SubscribeInvocation {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let lookahead = input.lookahead1();
        if lookahead.peek(Token![_]) {
            input.parse().map(Self::WithoutKey)
        }
        else {
            input.parse().map(Self::WithKey)
        }
    }
}

#[derive(syn_derive::Parse)]
#[derive(PartialEq, Eq)]
struct SubscribeInvocationWithoutKey {
    _underscore_token: Token![_],
    _fat_arrow_token: Token![=>],
    pattern: EventLoopPatternWithoutKey
}

#[derive(syn_derive::Parse)]
#[derive(PartialEq, Eq)]
struct SubscribeInvocationWithKey {
    query_id: syn::Ident,
    _colon_token: Token![:],
    query_type: syn::Type,
    _fat_arrow_token: Token![=>],
    pattern: EventLoopPatternWithKey
}

#[derive(PartialEq, Eq)]
struct EventLoopPatternWithKey {
    query_key: syn::Member,
    pattern: syn::Pat,
    output: Vec<syn::Member>
}

impl Parse for EventLoopPatternWithKey {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let mut pattern = syn::Pat::parse_single(input)?;
        let mut visitor = PatternVisitor::default();

        visitor.visit_pat_mut(&mut pattern);
        let key_no = visitor.keys.len();
        if key_no != 1 {
            return Err(input.error(format!(
                "The filter type subscribe! macro requires 1 key, but {key_no} were provided"
            )));
        }

        let output = visitor.other
            .into_iter()
            .filter_map(|other| if let None = other.colon_token { Some(other.member) } else { None })
            .collect();

        let query_key = visitor.keys[0].member.clone();
        Ok(Self {
            query_key,
            pattern,
            output
        })
    }
}

#[derive(PartialEq, Eq)]
struct EventLoopPatternWithoutKey {
    pattern: syn::Pat,
    output: Vec<syn::Member>
}

impl Parse for EventLoopPatternWithoutKey {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let mut pattern = syn::Pat::parse_single(input)?;
        let mut visitor = PatternVisitor::default();

        visitor.visit_pat_mut(&mut pattern);
        let key_no = visitor.keys.len();
        if key_no > 0 {
            return Err(input.error(format!(
                "You need to provide a concrete value to match the key against."
            )));
        }

        let output = visitor.other
            .into_iter()
            .filter_map(|other| if let None = other.colon_token { Some(other.member) } else { None })
            .collect();

        Ok(Self {
            pattern,
            output
        })
    }
}

#[derive(Default)]
struct PatternVisitor {
    keys: Vec<syn::FieldPat>,
    other: Vec<syn::FieldPat>
}

impl VisitMut for PatternVisitor {
    fn visit_field_pat_mut(&mut self, node: &mut syn::FieldPat) {
        let attrs: Vec<Attribute> = node.attrs
            .extract_if(.., |attr| attr.path().is_ident("key"))
            .collect();

        if !attrs.is_empty() {
            self.keys.push(node.clone());
        }
        else {
            self.other.push(node.clone());
        }
        
        visit_mut::visit_field_pat_mut(self, node);
    }
}

#[derive(Default)]
struct EventLoop {
    queues: Vec<SubscribeQueue>
}

impl EventLoop {
    fn get_queues_with_keys(&self) -> impl Iterator<Item = &SubscribeQueue> {
        self.queues
            .iter()
            .filter_map(|queue| {
                if let SubscribeInvocation::WithKey(_) = &queue.invocation {
                    Some(queue)
                }
                else {
                    None
                }
            })
    }
    
    fn get_invocations_with_keys(&self) -> impl Iterator<Item = &SubscribeInvocationWithKey> {
        self.get_queues_with_keys()
            .filter_map(|queue| {
                if let SubscribeInvocation::WithKey(invocation) = &queue.invocation {
                    Some(invocation)
                }
                else {
                    None
                }
            })
    }

    fn get_queues_without_keys(&self) -> impl Iterator<Item = &SubscribeQueue> {
        self.queues
            .iter()
            .filter_map(|queue| {
                if let SubscribeInvocation::WithoutKey(_) = &queue.invocation {
                    Some(queue)
                }
                else {
                    None
                }
            })
    }
    
    fn get_invocations_without_keys(&self) -> impl Iterator<Item = &SubscribeInvocationWithoutKey> {
        self.get_queues_without_keys()
            .filter_map(|queue| {
                if let SubscribeInvocation::WithoutKey(invocation) = &queue.invocation {
                    Some(invocation)
                }
                else {
                    None
                }
            })
    }
    
    fn append_pending_queries_struct(&self, tokens: &mut TokenStream) {
        let with_key_db_items = self.get_queues_with_keys()
            .map(|queue| tools::member_case(&queue.name));

        let with_key_db_types: Vec<_> = self.get_invocations_with_keys()
            .map(|invocation| &invocation.query_type)
            .collect();

        let with_key_db_item_buffer = self.get_queues_with_keys()
            .map(|queue| tools::member_case(&queue.name))
            .map(|item| format_ident!("{}_buffer", item));

        let without_key_db_items = self.get_queues_without_keys()
            .map(|queue| tools::member_case(&queue.name));

        let without_key_db_item_buffer = self.get_queues_without_keys()
            .map(|queue| tools::member_case(&queue.name))
            .map(|item| format_ident!("{}_buffer", item));

        // TODO - make the behaviour generic
        tokens.append_all(quote! {
            #[derive(Default)]
            pub(crate) struct PendingQueries {
                #(pub #with_key_db_items: HashMap<#with_key_db_types, tokio::sync::oneshot::Sender<libp2p::swarm::SwarmEvent<BehaviourEvent>>> ,)*
                #(pub #with_key_db_item_buffer: HashMap<#with_key_db_types, libp2p::swarm::SwarmEvent<BehaviourEvent>> ,)*
                #(pub #without_key_db_items: Option<tokio::sync::oneshot::Sender<libp2p::swarm::SwarmEvent<BehaviourEvent>>> ,)*
                #(pub #without_key_db_item_buffer: Vec<libp2p::swarm::SwarmEvent<BehaviourEvent>>),*
            }
        });
    }
    
    fn append_event_loop_struct(&self, tokens: &mut TokenStream) {
        tokens.append_all(quote! {
            pub(crate) struct EventLoop {
                pub swarm: libp2p::swarm::Swarm<Behaviour>,
                pub pending: PendingQueries,
                fn_receiver: tokio::sync::mpsc::Receiver<EventLoopFn>,
                cancel: tokio_util::sync::CancellationToken
            }
        })
    }

    fn append_event_loop_impl(&self, tokens: &mut TokenStream) {
        let key_names = self.get_queues_with_keys()
            .map(|queue| tools::member_case(&queue.name));

        let key_buffers = self.get_queues_with_keys()
            .map(|queue| tools::member_case(&queue.name))
            .map(|item| format_ident!("{item}_buffer"));

        let key_patterns = self.get_invocations_with_keys()
            .map(|invocation| &invocation.pattern.pattern);
        
        let keys = self.get_invocations_with_keys()
            .map(|invocation| &invocation.pattern.query_key);

        let other_names = self.get_queues_without_keys()
            .map(|queue| tools::member_case(&queue.name));

        let other_buffers = self.get_queues_without_keys()
            .map(|queue| tools::member_case(&queue.name))
            .map(|item| format_ident!("{item}_buffer"));

        let other_patterns = self.get_invocations_without_keys()
            .map(|invocation| &invocation.pattern.pattern);
        
        
        tokens.append_all(quote! {
            impl EventLoop {
                pub fn new(
                    swarm: libp2p::swarm::Swarm<Behaviour>,
                    fn_receiver: tokio::sync::mpsc::Receiver<EventLoopFn>,
                    cancel: tokio_util::sync::CancellationToken
                ) -> Self {
                    Self {
                        swarm,
                        fn_receiver,
                        pending: Default::default(),
                        cancel
                    }
                }

                pub(crate) async fn run(mut self) {
                    loop {
                        tokio::select! {
                            Some(event) = futures::stream::StreamExt::next(&mut self.swarm) => self.handle_event(event).await,
                            Some(func) = self.fn_receiver.recv() => func(&mut self),
                            _ = self.cancel.cancelled() => return
                        }
                    }
                }

                async fn handle_event(&mut self, event: libp2p::swarm::SwarmEvent<BehaviourEvent>) {
                    match &event {
                        #(#key_patterns => {
                            let out = self
                                .pending
                                .#key_names
                                .remove(&#keys);

                            if let Some(sender) = out {
                                let _ = sender.send(event);
                            }
                            else {
                                self.pending
                                    .#key_buffers
                                    .insert(*#keys, event);
                            }
                        })*
                        #(#other_patterns => {
                            let out = self
                                .pending
                                .#other_names
                                .take();
                            
                            if let Some(sender) = out {
                                let _ = sender.send(event);
                            }
                            else {
                                self.pending
                                    .#other_buffers
                                    .push(event);
                            }
                            
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
    invocation_count: u64,
    event_loop: EventLoop
}

impl VisitMut for SubscriberVisitor {
    fn visit_impl_item_fn_mut(&mut self, node: &mut syn::ImplItemFn) {
        self.current_fn = Some(node.sig.ident.clone());
        self.invocation_count = 0;
        visit_mut::visit_impl_item_fn_mut(self, node);
    }
    
    fn visit_expr_mut(&mut self, node: &mut syn::Expr) {
        if let syn::Expr::Macro(expr) = node {
            if expr.mac.path.is_ident("subscribe") {
                self.invocation_count += 1;
                let invocation: SubscribeInvocation = expr.mac
                    .parse_body()
                    .unwrap();
                let context = format_ident!(
                    "{}_{}", self.current_fn.as_ref().unwrap(), self.invocation_count
                );
                let queue = SubscribeQueue::new(context, invocation);
                *node = parse_quote!(#queue);
                if !self.event_loop.queues.contains(&queue) {
                    self.event_loop.queues.push(queue);
                }
            }
        }
        visit_mut::visit_expr_mut(self, node);
    }
}
