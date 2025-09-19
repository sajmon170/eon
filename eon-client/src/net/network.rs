use std::{
    collections::{hash_map, HashMap, HashSet},
    error::Error,
    time::Duration,
};

use futures::{prelude::*, stream::FuturesUnordered, StreamExt};
use libp2p::{
    core::{ConnectedPoint, Multiaddr},
    identify, identity::{self, PublicKey}, kad::{self, KadPeer, ProviderRecord},
    multiaddr::Protocol,
    noise,
    request_response::{self, OutboundRequestId, ProtocolSupport, ResponseChannel},
    swarm::{self, DialError, NetworkBehaviour, Swarm, SwarmEvent},
    tcp, yamux, PeerId, StreamProtocol,
};
use serde::{Deserialize, Serialize};
use tracing::{event, Level};

use tokio::sync::{mpsc, oneshot};

use objects::prelude::*;
use objects::system;
use objects::system::Hash;

use libp2p_invert::{event_subscriber, swarm_client};

use libp2p::kad::{QueryId, store::RecordStore};

#[derive(NetworkBehaviour)]
pub(crate) struct Behaviour {
    pub object_exchange: request_response::cbor::Behaviour<ObjectRpc, ObjectResponse>,
    pub fastkad: request_response::cbor::Behaviour<KadRequest, KadResponse>,
    pub data_stream: libp2p_stream::Behaviour,
    pub kademlia: kad::Behaviour<kad::store::MemoryStore>,
    pub identify: identify::Behaviour,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KadRequest {
    pub id: ObjectId
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub struct KadPeerData {
    pub id: PeerId,
    pub addrs: Vec<Multiaddr>
}

impl From<KadPeer> for KadPeerData {
    fn from(peer: KadPeer) -> Self {
        Self {
            id: peer.node_id,
            addrs: peer.multiaddrs
        }
    }
}

impl From<ProviderRecord> for KadPeerData {
    fn from(peer: ProviderRecord) -> Self {
        Self {
            id: peer.provider,
            addrs: peer.addresses
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct KadResponse {
    pub closer_peers: HashSet<KadPeerData>,
    pub provider_peers: HashSet<KadPeerData>,
    pub shortcut_peers: HashSet<KadPeerData>
}

// Simple file exchange protocol
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ObjectRpc(pub TypedObject);
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ObjectResponse(pub Vec<SignedObject>);

/// Creates the network client to interact with the network layer
pub(crate) async fn new(
    id_keys: identity::Keypair,
    is_bootstrap: bool,
) -> Result<Client, Box<dyn Error + Send + Sync>> {
    let peer_id = id_keys.public().to_peer_id();

    let mut swarm = libp2p::SwarmBuilder::with_existing_identity(id_keys.clone())
        .with_tokio()
        .with_tcp(
            tcp::Config::default(),
            noise::Config::new,
            yamux::Config::default,
        )?
        .with_behaviour(|key| Behaviour {
            kademlia: kad::Behaviour::new(
                peer_id,
                kad::store::MemoryStore::new(key.public().to_peer_id()),
            ),
            fastkad: request_response::cbor::Behaviour::new(
                [(
                    StreamProtocol::new("/fastkad/1"),
                    ProtocolSupport::Full,
                )],
                request_response::Config::default(),
            ),
            object_exchange: request_response::cbor::Behaviour::new(
                [(
                    StreamProtocol::new("/object-exchange/1"),
                    ProtocolSupport::Full,
                )],
                request_response::Config::default(),
            ),
            identify: identify::Behaviour::new(identify::Config::new(
                String::from("liberum/id/1.0.0"),
                key.public(),
            )),
            data_stream: libp2p_stream::Behaviour::new(),
        })?
        .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
        .build();

    if is_bootstrap {
        swarm.listen_on("/ip4/0.0.0.0/tcp/22137".parse()?)?;
    }

    swarm
        .behaviour_mut()
        .kademlia
        .set_mode(Some(kad::Mode::Server));

    if !is_bootstrap {
        let bootstrap_id: PeerId = "12D3KooWPjceQrSwdWXPyLLeABRXmuqt69Rg3sBYbU1Nft9HyQ6X"
            .parse()
            .unwrap();
        let bootstrap_addr: Multiaddr = "/ip4/127.0.0.1/tcp/22137".parse()?;

        swarm.behaviour_mut().kademlia.add_address(
            &bootstrap_id,
            bootstrap_addr.clone().with(Protocol::P2p(bootstrap_id)),
        );
    } else {
        println!("Bootstrap node!");
    }

    println!("Started node");

    let control = swarm.behaviour().data_stream.new_control();

    Ok(Client::new(swarm, id_keys).await)
}

// Idea: add a #[Constructor] attribute macro to detect user constructors
// and expand them with necessary init stuff

#[swarm_client(Behaviour)]
#[derive(Clone)]
pub(crate) struct Client {
    keys: identity::Keypair,
}

#[event_subscriber(Behaviour)]
impl Client {
    pub(crate) async fn on_identify_received(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let (peer_id, info) = subscribe!(_ => SwarmEvent::Behaviour(BehaviourEvent::Identify(
            identify::Event::Received { peer_id, info, .. }
        )))
        .await?;

        self.register(move |swarm| {
            for addr in info.listen_addrs {
                event!(
                    Level::INFO,
                    "Found new listen addr: {addr} for peer: {peer_id}"
                );
                swarm.behaviour_mut().kademlia.add_address(&peer_id, addr);
            }
        });

        Ok(())
    }

    pub(crate) async fn bootstrap(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let _ = subscribe!(_ => SwarmEvent::Behaviour(BehaviourEvent::Kademlia(
            kad::Event::OutboundQueryProgressed {
                result: kad::QueryResult::Bootstrap(result),
                ..
            }
        )))
        .await?;

        Ok(())
    }

    pub fn get_keys(&self) -> &identity::Keypair {
        &self.keys
    }

    pub fn get_peer_id(&self) -> PeerId {
        PeerId::from_public_key(&self.get_keys().public())
    }

    /// Listen for incoming connections on the given address.
    pub(crate) async fn start_listening(
        &self,
        addr: Multiaddr,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let status = self.register(move |swarm| swarm.listen_on(addr)).await?;

        match status {
            Ok(_) => Ok(()),
            Err(e) => Err(Box::new(e)),
        }
    }

    async fn get_connection_established(
        &self,
        id: PeerId,
    ) -> Result<ConnectedPoint, Box<dyn Error + Send + Sync>> {
        let out = subscribe!(id: PeerId => SwarmEvent::ConnectionEstablished {
            #[key] peer_id,
            endpoint,
            ..
        })
        .await?;

        Ok(out)
    }

    // This function might hang indefinitely!
    async fn get_outgoing_connection_error(
        &self,
        id: PeerId,
    ) -> Result<DialError, Box<dyn Error + Send + Sync>> {
        let id = Some(id);
        let out = subscribe!(id: Option<PeerId> => SwarmEvent::OutgoingConnectionError {
            #[key] peer_id,
            error,
            ..
        })
        .await?;

        Ok(out)
    }

    /// Dial the given peer at the given address.
    pub(crate) async fn dial(
        &self,
        peer_id: PeerId,
        peer_addr: Multiaddr,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let _ = self
            .register(move |swarm| {
                swarm
                    .behaviour_mut()
                    .kademlia
                    .add_address(&peer_id, peer_addr.clone());

                swarm.dial(peer_addr.with(Protocol::P2p(peer_id.clone())))
            })
            .await?;

        loop {
            tokio::select! {
                Ok(endpoint) = self.get_connection_established(peer_id.clone()) => {
                    if endpoint.is_dialer() {
                        break Ok(())
                    }
                },
                Ok(error) = self.get_outgoing_connection_error(peer_id.clone()) => {
                    break Err(error)
                }
            }
        }?;

        Ok(())
    }

    /// Advertise the local node as the provider of the given file on the DHT.
    pub(crate) async fn start_providing(
        &self,
        object: ObjectId,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let query_id = self
            .register(move |swarm| {
                swarm
                    .behaviour_mut()
                    .kademlia
                    .start_providing(Vec::from(object).into())
                    .expect("No store error.")
            })
            .await?;

        let _ = subscribe!(query_id: QueryId => SwarmEvent::Behaviour(BehaviourEvent::Kademlia(
            kad::Event::OutboundQueryProgressed {
                #[key] id,
                result: kad::QueryResult::StartProviding(_),
                ..
            }
        )))
        .await?;

        Ok(())
    }

    /// Find the providers for the given file on the DHT.
    pub(crate) async fn get_providers(
        &self,
        object: ObjectId,
    ) -> Result<HashSet<PeerId>, Box<dyn Error + Send + Sync>> {
        let query_id = self
            .register(move |swarm| {
                swarm
                    .behaviour_mut()
                    .kademlia
                    .get_providers(Vec::from(object).into())
            })
            .await?;

        let providers =
            subscribe!(query_id: QueryId => SwarmEvent::Behaviour(BehaviourEvent::Kademlia(
                kad::Event::OutboundQueryProgressed {
                    #[key] id,
                    result:
                        kad::QueryResult::GetProviders(Ok(kad::GetProvidersOk::FoundProviders {
                            providers,
                            ..
                        })),
                    ..
                }
            )))
            .await?;

        Ok(providers)
    }

    async fn update_routing_table(&self, response: KadResponse) {
        let mut all_peers = HashSet::new();
        all_peers.extend(response.closer_peers);
        all_peers.extend(response.provider_peers);
        all_peers.extend(response.shortcut_peers);

        let swarm_tasks = all_peers
            .into_iter()
            .map(|peer| self.register(move |swarm| {
                if let Some(addr) = peer.addrs.first() {
                    swarm
                        .behaviour_mut()
                        .kademlia
                        .add_address(&peer.id, addr.clone());
                }
            }));

        futures::future::join_all(swarm_tasks).await;
    }

    /// Send a Fastkad RPC
    pub(crate) async fn send_fastkad_rpc(
        &self,
        peer: KadPeerData,
        request: KadRequest,
    ) -> Result<KadResponse, Box<dyn Error + Send + Sync>> {
        let request_id = self
            .register(move |swarm| {
                swarm
                    .behaviour_mut()
                    .fastkad
                    .send_request_with_addresses(&peer.id, request, peer.addrs)
            })
            .await?;

        let id = request_id.clone();
        let response_future = subscribe!(
            id: OutboundRequestId => SwarmEvent::Behaviour(BehaviourEvent::Fastkad(
                request_response::Event::Message {
                    message: request_response::Message::Response {
                        #[key] request_id,
                        response
                    },
                    ..
                },
                ..
            ))
        );

        let error_future = subscribe!(
            request_id: OutboundRequestId => SwarmEvent::Behaviour(BehaviourEvent::Fastkad(
                request_response::Event::OutboundFailure {
                    #[key] request_id,
                    error,
                    ..
                }
            ))
        );

        tokio::select! {
            Ok(response) = response_future => {
                self.update_routing_table(response.clone()).await;
                Ok(response)
            },
            Ok(error) = error_future => Err(Box::new(error))
        }
    }

    /// Respond to Fastkad RPC
    pub(crate) async fn respond_fastkad_rpc(
        &self,
        response: KadResponse,
        channel: ResponseChannel<KadResponse>,
    ) {
        let _ = self
            .register(move |swarm| {
                swarm
                    .behaviour_mut()
                    .fastkad
                    .send_response(channel, response)
                    .expect("Connection to peer to be still open.");
            })
            .await;
    }

    /// Send an object RPC
    pub(crate) async fn send_rpc(
        &self,
        peer: KadPeerData,
        rpc: TypedObject,
    ) -> Result<Vec<SignedObject>, Box<dyn Error + Send + Sync>> {
        let request_id = self
            .register(move |swarm| {
                swarm
                    .behaviour_mut()
                    .object_exchange
                    .send_request_with_addresses(&peer.id, ObjectRpc(rpc), peer.addrs)
            })
            .await?;

        let id = request_id.clone();
        let response_future = subscribe!(
            id: OutboundRequestId => SwarmEvent::Behaviour(BehaviourEvent::ObjectExchange(
                request_response::Event::Message {
                    message: request_response::Message::Response {
                        #[key] request_id,
                        response
                    },
                    ..
                },
                ..
            ))
        );

        let error_future = subscribe!(
            request_id: OutboundRequestId => SwarmEvent::Behaviour(BehaviourEvent::ObjectExchange(
                request_response::Event::OutboundFailure {
                    #[key] request_id,
                    error,
                    ..
                }
            ))
        );

        tokio::select! {
            Ok(response) = response_future => Ok(response.0),
            Ok(error) = error_future => Err(Box::new(error))
        }
    }

    pub(crate) async fn on_object_request(
        &self,
    ) -> Result<(TypedObject, ResponseChannel<ObjectResponse>), Box<dyn Error + Send + Sync>> {
        let (request, channel) = subscribe!(
            _ => SwarmEvent::Behaviour(BehaviourEvent::ObjectExchange(
                request_response::Event::Message {
                    message: request_response::Message::Request {
                        request,
                        channel,
                        ..
                    },
                    ..
                },
                ..
            ))
        )
        .await?;

        Ok((request.0, channel))
    }

    pub(crate) async fn on_fastkad_request(
        &self,
    ) -> Result<(PeerId, KadRequest, ResponseChannel<KadResponse>), Box<dyn Error + Send + Sync>> {
        let (peer, request, channel) = subscribe!(
            _ => SwarmEvent::Behaviour(BehaviourEvent::Fastkad(
                request_response::Event::Message {
                    peer,
                    message: request_response::Message::Request {
                        request,
                        channel,
                        ..
                    },
                    ..
                },
                ..
            ))
        )
        .await?;

        Ok((peer, request, channel))
    }

    /// Respond to an object RPC
    pub(crate) async fn respond_rpc(
        &self,
        response: Vec<SignedObject>,
        channel: ResponseChannel<ObjectResponse>,
    ) {
        let _ = self
            .register(move |swarm| {
                swarm
                    .behaviour_mut()
                    .object_exchange
                    .send_response(channel, ObjectResponse(response))
                    .expect("Connection to peer to be still open.");
            })
            .await;
    }

    pub(crate) async fn find_closest_local_peers(&self, id: ObjectId, source: PeerId) -> Vec<KadPeerData> {
        self.register(move |swarm| {
            swarm
                .behaviour_mut()
                .kademlia
                .find_closest_local_peers(&Vec::from(id).into(), &source)
                .map(|peer| KadPeerData::from(peer))
                .collect()
        }).await.unwrap()
    }

    pub(crate) async fn find_providers(&self, id: ObjectId) -> Vec<KadPeerData> {
        let key = Vec::from(id).into();
        self.register(move |swarm| {
            swarm
                .behaviour_mut()
                .kademlia
                .store_mut()
                .providers(&key)
                .iter()
                .filter_map(|record| (record.key == key).then(|| record.clone().into()))
                .collect()
        }).await.unwrap()
    }

    pub(crate) async fn open_stream(&self, hash: Hash, id: PeerId) {
        //self.streams.open_stream(hash, id)
    }
}
