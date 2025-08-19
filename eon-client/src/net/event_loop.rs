use std::{
    collections::{hash_map, HashMap, HashSet},
    error::Error,
    time::Duration,
};

use futures::{
    channel::{mpsc, oneshot},
    prelude::*,
    StreamExt,
};
use libp2p::{
    core::Multiaddr, identify, identity, kad::{self, QueryId}, multiaddr::Protocol, noise, request_response::{self, OutboundRequestId, ProtocolSupport, ResponseChannel}, swarm::{NetworkBehaviour, Swarm, SwarmEvent}, tcp, yamux, PeerId, StreamProtocol
};
use serde::{Deserialize, Serialize};

use tracing::{Level, event};

use objects::prelude::*;

pub enum SwarmOutput {
    PeerId(PeerId),
    QueryId(QueryId),
    OutboundRequestId(OutboundRequestId)
}

pub type SwarmFn = Box<dyn FnOnce(&mut Swarm<Behaviour>) + Send + Sync>;
pub type EventLoopFn = Box<dyn FnOnce(&mut EventLoop) + Send + Sync>;

#[derive(Default)]
pub(crate) struct PendingQueries {
    pub dial: HashMap<PeerId, oneshot::Sender<Result<(), Box<dyn Error + Send + Sync>>>>,
    pub start_providing: HashMap<kad::QueryId, oneshot::Sender<()>>,
    pub get_providers: HashMap<kad::QueryId, oneshot::Sender<HashSet<PeerId>>>,
    pub object_rpc: HashMap<OutboundRequestId,
                                oneshot::Sender<Result<Vec<SignedObject>, Box<dyn Error + Send + Sync>>>>,
    pub bootstrap_listener: Option<oneshot::Sender<()>>
}

pub(crate) struct EventLoop {
    pub swarm: Swarm<Behaviour>,
    pub pending: PendingQueries,
    command_receiver: mpsc::Receiver<Command>,
    fn_receiver: mpsc::Receiver<EventLoopFn>,
    event_sender: mpsc::Sender<Event>
}

impl EventLoop {
    pub fn new(
        swarm: Swarm<Behaviour>,
        command_receiver: mpsc::Receiver<Command>,
        event_sender: mpsc::Sender<Event>,
        fn_receiver: mpsc::Receiver<EventLoopFn>
    ) -> Self {
        Self {
            swarm,
            command_receiver,
            event_sender,
            fn_receiver,
            pending: Default::default()
        }
    }

    pub(crate) async fn run(mut self) {
        loop {
            tokio::select! {
                event = self.swarm.select_next_some() => self.handle_event(event).await,
                command = self.command_receiver.next() => match command {
                    Some(c) => self.handle_command(c).await,
                    // Command channel closed, thus shutting down the network event loop.
                    None=>  return,
                },
                func = self.fn_receiver.select_next_some() => func(&mut self)
            }
        }
    }

    async fn handle_event(&mut self, event: SwarmEvent<BehaviourEvent>) {
        match event {
            SwarmEvent::Behaviour(BehaviourEvent::Identify(
                identify::Event::Sent { connection_id, peer_id })) => {}
            SwarmEvent::Behaviour(BehaviourEvent::Identify(
                identify::Event::Received { connection_id, peer_id, info }
            )) => {
                for addr in info.listen_addrs {
                    event!(Level::INFO,
                           "Found new listen addr: {addr} for peer: {peer_id}");
                    self.swarm.behaviour_mut().kademlia.add_address(&peer_id, addr);
                }
            }
            SwarmEvent::Behaviour(BehaviourEvent::Kademlia(
                kad::Event::OutboundQueryProgressed {
                    id,
                    result: kad::QueryResult::StartProviding(_),
                    ..
                },
            )) => {
                let sender: oneshot::Sender<()> = self
                    .pending
                    .start_providing
                    .remove(&id)
                    .expect("Completed query to be previously pending.");
                let _ = sender.send(());
            }
            SwarmEvent::Behaviour(BehaviourEvent::Kademlia(
                kad::Event::OutboundQueryProgressed {
                    id,
                    result:
                        kad::QueryResult::GetProviders(Ok(kad::GetProvidersOk::FoundProviders {
                            providers,
                            ..
                        })),
                    ..
                },
            )) => {
                if let Some(sender) = self.pending.get_providers.remove(&id) {
                    sender.send(providers).expect("Receiver not to be dropped");

                    // Finish the query. We are only interested in the first result.
                    self.swarm
                        .behaviour_mut()
                        .kademlia
                        .query_mut(&id)
                        .unwrap()
                        .finish();
                }
            }
            SwarmEvent::Behaviour(BehaviourEvent::Kademlia(
                kad::Event::OutboundQueryProgressed {
                    result:
                        kad::QueryResult::GetProviders(Ok(
                            kad::GetProvidersOk::FinishedWithNoAdditionalRecord { .. },
                        )),
                    ..
                },
            )) => {}
            SwarmEvent::Behaviour(BehaviourEvent::Kademlia(
                kad::Event::OutboundQueryProgressed {
                    result: kad::QueryResult::Bootstrap(result),
                    ..
                }
            )) => {
                if result.is_err() {
                    return;
                }

                if let Some(sender) = self.pending.bootstrap_listener.take() {
                    let _ = sender.send(());
                }
            },
            SwarmEvent::Behaviour(BehaviourEvent::Kademlia(_)) => {}
            SwarmEvent::Behaviour(BehaviourEvent::ObjectExchange(
                request_response::Event::Message { message, .. },
            )) => match message {
                request_response::Message::Request {
                    request, channel, ..
                } => {
                    self.event_sender
                        .send(Event::InboundRpc {
                            rpc: request.0,
                            channel,
                        })
                        .await
                        .expect("Event receiver not to be dropped.");
                }
                request_response::Message::Response {
                    request_id,
                    response,
                } => {
                    let _ = self
                        .pending
                        .object_rpc
                        .remove(&request_id)
                        .expect("Request to still be pending.")
                        .send(Ok(response.0));
                }
            },
            SwarmEvent::Behaviour(BehaviourEvent::ObjectExchange(
                request_response::Event::OutboundFailure {
                    request_id, error, ..
                },
            )) => {
                let _ = self
                    .pending
                    .object_rpc
                    .remove(&request_id)
                    .expect("Request to still be pending.")
                    .send(Err(Box::new(error)));
            }
            SwarmEvent::Behaviour(BehaviourEvent::ObjectExchange(
                request_response::Event::ResponseSent { .. },
            )) => {}
            SwarmEvent::NewListenAddr { address, .. } => {
                let local_peer_id = *self.swarm.local_peer_id();
                event!(
                    Level::INFO,
                    "Local node is listening on {:?}",
                    address.with(Protocol::P2p(local_peer_id))
                );
            }
            SwarmEvent::IncomingConnection { .. } => {}
            SwarmEvent::ConnectionEstablished {
                peer_id, endpoint, ..
            } => {
                event!(Level::INFO, "Established incoming from {peer_id}");
                if endpoint.is_dialer() {
                    if let Some(sender) = self.pending.dial.remove(&peer_id) {
                        let _ = sender.send(Ok(()));
                    }
                }
            }
            SwarmEvent::ConnectionClosed { .. } => {}
            SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                event!(Level::INFO, "Outgoing error: {error}");

                if let Some(peer_id) = peer_id {
                    if let Some(sender) = self.pending.dial.remove(&peer_id) {
                        let _ = sender.send(Err(Box::new(error)));
                    }
                }
            }
            SwarmEvent::IncomingConnectionError { error, send_back_addr, .. } => {
                event!(Level::INFO, "Incoming error: {error} from {send_back_addr}")
            }
            SwarmEvent::Dialing {
                peer_id: Some(peer_id),
                ..
            } => {
                event!(Level::INFO, "Dialing {peer_id}");
            }
            _ => {}
        }
    }

    async fn handle_command(&mut self, command: Command) {
        match command {
            Command::NotifyAfterBootstrap { sender } => {
                self.pending.bootstrap_listener = Some(sender);
            }
            Command::StartListening { addr, sender } => {
                let _ = match self.swarm.listen_on(addr) {
                    Ok(_) => sender.send(Ok(())),
                    Err(e) => sender.send(Err(Box::new(e))),
                };
            }
        }
    }
}

#[derive(NetworkBehaviour)]
pub(crate) struct Behaviour {
    pub object_exchange: request_response::cbor::Behaviour<ObjectRpc, ObjectResponse>,
    pub data_stream: libp2p_stream::Behaviour,
    pub kademlia: kad::Behaviour<kad::store::MemoryStore>,
    pub identify: identify::Behaviour
}

#[derive(Debug)]
pub(crate) enum Command {
    NotifyAfterBootstrap {
        sender: oneshot::Sender<()>
    },
    StartListening {
        addr: Multiaddr,
        sender: oneshot::Sender<Result<(), Box<dyn Error + Send + Sync>>>,
    },
}

#[derive(Debug)]
pub(crate) enum Event {
    InboundRpc {
        rpc: TypedObject,
        channel: ResponseChannel<ObjectResponse>,
    },
}

// Simple file exchange protocol
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ObjectRpc(pub TypedObject);
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ObjectResponse(pub Vec<SignedObject>);
