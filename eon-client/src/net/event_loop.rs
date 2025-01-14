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
    core::Multiaddr,
    identity, kad,
    multiaddr::Protocol,
    noise,
    request_response::{self, OutboundRequestId, ProtocolSupport, ResponseChannel},
    swarm::{NetworkBehaviour, Swarm, SwarmEvent},
    tcp, yamux, PeerId, StreamProtocol,
    identify
};
use serde::{Deserialize, Serialize};

use tracing::{Level, event};

use objects::prelude::*;

pub(crate) struct EventLoop {
    swarm: Swarm<Behaviour>,
    command_receiver: mpsc::Receiver<Command>,
    event_sender: mpsc::Sender<Event>,
    pending_dial: HashMap<PeerId, oneshot::Sender<Result<(), Box<dyn Error + Send>>>>,
    pending_start_providing: HashMap<kad::QueryId, oneshot::Sender<()>>,
    pending_get_providers: HashMap<kad::QueryId, oneshot::Sender<HashSet<PeerId>>>,
    pending_object_rpc: HashMap<OutboundRequestId,
                                oneshot::Sender<Result<Vec<SignedObject>, Box<dyn Error + Send>>>>,
    bootstrap_listener: Option<oneshot::Sender<()>>
}

impl EventLoop {
    pub fn new(
        swarm: Swarm<Behaviour>,
        command_receiver: mpsc::Receiver<Command>,
        event_sender: mpsc::Sender<Event>,
    ) -> Self {
        Self {
            swarm,
            command_receiver,
            event_sender,
            pending_dial: Default::default(),
            pending_start_providing: Default::default(),
            pending_get_providers: Default::default(),
            pending_object_rpc: Default::default(),
            bootstrap_listener: Default::default()
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
                    .pending_start_providing
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
                if let Some(sender) = self.pending_get_providers.remove(&id) {
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

                if let Some(sender) = self.bootstrap_listener.take() {
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
                        .pending_object_rpc
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
                    .pending_object_rpc
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
                    if let Some(sender) = self.pending_dial.remove(&peer_id) {
                        let _ = sender.send(Ok(()));
                    }
                }
            }
            SwarmEvent::ConnectionClosed { .. } => {}
            SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                event!(Level::INFO, "Outgoing error: {error}");

                if let Some(peer_id) = peer_id {
                    if let Some(sender) = self.pending_dial.remove(&peer_id) {
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
                self.bootstrap_listener = Some(sender);
            }
            Command::StartListening { addr, sender } => {
                let _ = match self.swarm.listen_on(addr) {
                    Ok(_) => sender.send(Ok(())),
                    Err(e) => sender.send(Err(Box::new(e))),
                };
            }
            Command::Dial {
                peer_id,
                peer_addr,
                sender,
            } => {
                if let hash_map::Entry::Vacant(e) = self.pending_dial.entry(peer_id) {
                    self.swarm
                        .behaviour_mut()
                        .kademlia
                        .add_address(&peer_id, peer_addr.clone());
                    match self.swarm.dial(peer_addr.with(Protocol::P2p(peer_id))) {
                        Ok(()) => {
                            e.insert(sender);
                        }
                        Err(e) => {
                            let _ = sender.send(Err(Box::new(e)));
                        }
                    }
                } else {
                    todo!("Already dialing peer.");
                }
            }
            Command::StartProviding { object, sender } => {
                let query_id = self
                    .swarm
                    .behaviour_mut()
                    .kademlia
                    .start_providing(Vec::from(object).into())
                    .expect("No store error.");
                self.pending_start_providing.insert(query_id, sender);
            }
            Command::GetProviders { object, sender } => {
                let query_id = self
                    .swarm
                    .behaviour_mut()
                    .kademlia
                    .get_providers(Vec::from(object).into());
                self.pending_get_providers.insert(query_id, sender);
            }
            Command::SendRpc {
                rpc,
                peer,
                sender,
            } => {
                let request_id = self
                    .swarm
                    .behaviour_mut()
                    .object_exchange
                    .send_request(&peer, ObjectRpc(rpc));
                self.pending_object_rpc.insert(request_id, sender);
            }
            Command::RespondRpc { response, channel } => {
                self.swarm
                    .behaviour_mut()
                    .object_exchange
                    .send_response(channel, ObjectResponse(response))
                    .expect("Connection to peer to be still open.");
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
        sender: oneshot::Sender<Result<(), Box<dyn Error + Send>>>,
    },
    Dial {
        peer_id: PeerId,
        peer_addr: Multiaddr,
        sender: oneshot::Sender<Result<(), Box<dyn Error + Send>>>,
    },
    StartProviding {
        object: ObjectId,
        sender: oneshot::Sender<()>,
    },
    GetProviders {
        object: ObjectId,
        sender: oneshot::Sender<HashSet<PeerId>>,
    },
    SendRpc {
        rpc: TypedObject,
        peer: PeerId,
        sender: oneshot::Sender<Result<Vec<SignedObject>, Box<dyn Error + Send>>>,
    },
    RespondRpc {
        response: Vec<SignedObject>,
        channel: ResponseChannel<ObjectResponse>,
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
pub(crate) struct ObjectRpc(TypedObject);
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ObjectResponse(Vec<SignedObject>);
