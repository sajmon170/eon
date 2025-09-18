use crate::{
    app::{repl::*, state::AppStateHandle},
    net::network::{Client, KadRequest, KadResponse},
};
use anyhow::Result;
use base64::prelude::*;
use futures::{prelude::*, stream::FuturesUnordered, StreamExt};
use libp2p::{
    core::Multiaddr,
    identity::{self, Keypair, PeerId},
    multiaddr::Protocol,
};
use objects::{prelude::*, system};
use std::{collections::HashSet, error::Error, fs::File, io::Write, path::PathBuf, sync::Mutex};
use tokio::{sync::mpsc, task::spawn};
use tracing::{event, Level};

pub enum AppStatus {
    Running,
    Done,
}

struct AppController {
    state: AppStateHandle,
    network_client: Client,
    rx: mpsc::Receiver<Command>,
}

impl AppController {
    async fn run(&mut self) {
        loop {
            tokio::select! {
                //Some(event) = self.network_events.next() => self.handle_event(event).await.unwrap(),
                Ok(_) = self.network_client.on_identify_received() => { },
                Ok((rpc, channel)) = self.network_client.on_object_request() => {
                    event!(Level::INFO, "Responding to request.");

                    if let Some(objects) = self.state.rpc(rpc).await {
                        self.network_client.respond_rpc(objects, channel).await;
                    }
                }
                Some(cmd) = self.rx.recv() => { self.handle_command(cmd).await.unwrap(); }
            }
        }
    }

    async fn get_first_fastkad_response(&self, obj: ObjectId, peers: HashSet<PeerId>)
        -> Option<KadResponse> {
        peers
            .into_iter()
            .map(|peer| self.network_client
                    .send_fastkad_rpc(peer, KadRequest { id: obj }))
            .collect::<FuturesUnordered<_>>()
            .next()
            .await?.ok()
    }

    async fn drive_query_for_single_peer(&self, obj: ObjectId, peer: PeerId, filter: Mutex<HashSet<PeerId>>)
                                         -> Option<HashSet<PeerId>> {
        let mut peers_to_ask = HashSet::from([peer]);

        while let Some(out) = self.get_first_fastkad_response(obj, peers_to_ask).await {
            peers_to_ask = out.closer_peers;
            
            if !out.shortcut_peers.is_empty() {
                return Some(out.shortcut_peers)
            }
            if !out.provider_peers.is_empty() {
                return Some(out.provider_peers)
            }
        }

        None
    }

    async fn get_providers(&self, obj_id: ObjectId) -> Result<HashSet<PeerId>, Box<dyn Error + Send + Sync>>{
        //self.network_client.get
        let my_id = self.network_client.get_peer_id();
        let mut closest = self.network_client.find_closest_local_peers(obj_id, my_id);
        Ok(Default::default())
    }

    async fn handle_command(
        &mut self,
        cmd: Command,
    ) -> Result<AppStatus, Box<dyn Error + Send + Sync>> {
        let event: Option<AppStatus> = match cmd {
            Command::Provide { path } => {
                let file = BinaryFile::new(&path);
                let serialized = file
                    .make_typed()
                    .sign(self.network_client.get_keys())
                    .unwrap();
                let obj_id = serialized.get_object_id();

                println!("Providing: {}", BASE64_STANDARD.encode(&obj_id));

                self.state.add(serialized).await;
                self.network_client.start_providing(obj_id).await;

                None
            }
            Command::Get { name } => {
                let id: ObjectId = BASE64_STANDARD.decode(&name).unwrap().try_into().unwrap();

                let providers = self.network_client.get_providers(id.clone()).await?;
                if providers.is_empty() {
                    return Err(format!("Could not find provider for file {name}.").into());
                }

                let requests = providers.into_iter().map(|p| {
                    event!(Level::INFO, "Found provider: {p}");
                    let mut network_client = self.network_client.clone();

                    let rpc = GetObject::new(id).make_typed();
                    async move { network_client.send_rpc(p, rpc).await }.boxed()
                });

                let results = futures::future::select_ok(requests)
                    .await
                    .map_err(|_| "None of the providers returned file.")?
                    .0;

                let file = results
                    .into_iter()
                    .filter(|file| file.get_object_id() == id)
                    .next()
                    .unwrap();
                let file = system::deserialize::<BinaryFile>(&file.get_data());

                let path = dirs::download_dir().unwrap();
                println!(
                    "Saving {} to {}",
                    file.filename,
                    path.as_os_str().to_str().unwrap()
                );
                file.save(&path);

                None
            }

            Command::Quit => Some(AppStatus::Done),
        };

        Ok(event.unwrap_or(AppStatus::Running))
    }
}

pub struct AppControllerHandle {
    tx: mpsc::Sender<Command>,
}

impl AppControllerHandle {
    pub fn new(network_client: Client) -> Self {
        let (tx, rx) = mpsc::channel(32);

        let mut mgr = AppController {
            state: AppStateHandle::new(),
            network_client,
            rx,
        };

        spawn(async move {
            mgr.run().await;
        });

        Self { tx }
    }

    pub async fn send(&mut self, cmd: Command) {
        let _ = self.tx.send(cmd).await;
    }
}
