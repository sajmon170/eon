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
    identify,
};
use serde::{Deserialize, Serialize};

use crate::event_loop::*;
use crate::core::*;

pub type EventStream = mpsc::Receiver<Event>;

/// Creates the network components, namely:
///
/// - The network client to interact with the network layer from anywhere within your application.
///
/// - The network event stream, e.g. for incoming requests.
///
/// - The network task driving the network itself.
pub(crate) async fn new(
    id_keys: identity::Keypair,
    is_bootstrap: bool
) -> Result<(Client, EventStream, EventLoop), Box<dyn Error>> {
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
            object_exchange: request_response::cbor::Behaviour::new(
                [(
                    StreamProtocol::new("/object-exchange/1"),
                    ProtocolSupport::Full,
                )],
                request_response::Config::default(),
            ),
            identify: identify::Behaviour::new(identify::Config::new(
                String::from("liberum/id/1.0.0"),
                key.public()
            )),
            data_stream: libp2p_stream::Behaviour::new()
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
            .parse().unwrap();
        let bootstrap_addr: Multiaddr = "/ip4/127.0.0.1/tcp/22137".parse()?;

        swarm
            .behaviour_mut()
            .kademlia
            .add_address(&bootstrap_id, bootstrap_addr.clone().with(Protocol::P2p(bootstrap_id)));
    }
    else {
        println!("Bootstrap node!");
    }

    println!("Started node");

    let (command_sender, command_receiver) = mpsc::channel(0);
    let (event_sender, event_receiver) = mpsc::channel(0);

    Ok((
        Client {
            keys: id_keys,
            sender: command_sender
        },
        event_receiver,
        EventLoop::new(swarm, command_receiver, event_sender),
    ))
}

#[derive(Clone)]
pub(crate) struct Client {
    keys: identity::Keypair,
    sender: mpsc::Sender<Command>
}

impl Client {
    pub(crate) async fn bootstrap(&mut self) -> Result<(), Box<dyn Error>> {
        let (sender, receiver) = oneshot::channel::<()>();

        let _ = self.sender
            .send(Command::NotifyAfterBootstrap { sender })
            .await;

        receiver.await.unwrap();

        Ok(())
    }

    pub fn get_keys(&self) -> &identity::Keypair {
        &self.keys
    }
    
    /// Listen for incoming connections on the given address.
    pub(crate) async fn start_listening(
        &mut self,
        addr: Multiaddr,
    ) -> Result<(), Box<dyn Error + Send>> {
        let (sender, receiver) = oneshot::channel();
        let _ = self.sender
            .send(Command::StartListening { addr, sender })
            .await;
        
        receiver.await.expect("Sender not to be dropped.")
    }

    /// Dial the given peer at the given address.
    pub(crate) async fn dial(
        &mut self,
        peer_id: PeerId,
        peer_addr: Multiaddr,
    ) -> Result<(), Box<dyn Error + Send>> {
        let (sender, receiver) = oneshot::channel();
        let _ = self.sender
            .send(Command::Dial {
                peer_id,
                peer_addr,
                sender,
            })
            .await;
        receiver.await.expect("Sender not to be dropped.")
    }

    /// Advertise the local node as the provider of the given file on the DHT.
    pub(crate) async fn start_providing(&mut self, object: ObjectId) {
        let (sender, receiver) = oneshot::channel();
        let _ = self.sender
            .send(Command::StartProviding { object, sender })
            .await;
        receiver.await.expect("Sender not to be dropped.");
    }

    /// Find the providers for the given file on the DHT.
    pub(crate) async fn get_providers(&mut self, object: ObjectId) -> HashSet<PeerId> {
        let (sender, receiver) = oneshot::channel();
        let _ = self.sender
            .send(Command::GetProviders { object, sender })
            .await;
        receiver.await.expect("Sender not to be dropped.")
    }

    /// Request the content of the given file from the given peer.
    pub(crate) async fn request_file(
        &mut self,
        peer: PeerId,
        object: ObjectId,
    ) -> Result<SignedObject, Box<dyn Error + Send>> {
        let (sender, receiver) = oneshot::channel();
        let _ = self.sender
            .send(Command::RequestObject {
                object,
                peer,
                sender,
            })
            .await;
        receiver.await.expect("Sender not be dropped.")
    }

    /// Respond with the provided file content to the given request.
    pub(crate) async fn respond_file(
        &mut self,
        object: SignedObject,
        channel: ResponseChannel<ObjectResponse>,
    ) {
        let _ = self.sender
            .send(Command::RespondObject { object, channel })
            .await;
    }
}
