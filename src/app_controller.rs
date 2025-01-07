use crate::event_loop::{Event, self};
use crate::network::Client;
use futures::prelude::*;
use crate::repl::*;
use crate::app_state::AppStateHandle;
use crate::core::*;
use crate::testing_obj::BinaryFile;

use std::{error::Error, io::Write, path::PathBuf, fs::File};

use futures::{prelude::*, StreamExt};
use libp2p::{core::Multiaddr, multiaddr::Protocol, identity::{Keypair, self}};
use tokio::{task::spawn, sync::mpsc};
use tracing::{Level, event};
use anyhow::Result;
use base64::prelude::*;
use crate::system;

use crate::network::EventStream;

pub enum AppStatus {
    Running,
    Done
}

struct AppController {
    state: AppStateHandle,
    network_client: Client,
    network_events: EventStream,
    rx: mpsc::Receiver<Command>
}

impl AppController {
    async fn run(&mut self) {
        loop {
            tokio::select! {
                Some(event) = self.network_events.next() => self.handle_event(event).await.unwrap(),
                Some(cmd) = self.rx.recv() => { self.handle_command(cmd).await.unwrap(); }
            }
        }
    }

    async fn handle_event(&mut self, event: Event) -> Result<(), Box<dyn Error>> {
        match event {
            event_loop::Event::InboundRequest { request, channel } => {
                event!(Level::INFO, "Responding to request.");

                if let Some(object) = self.state.get(request).await {
                    self.network_client
                        .respond_file(object, channel)
                        .await;
                }
            }
            e => todo!("{:?}", e),
        };

        Ok(())
    }

    async fn handle_command(&mut self, cmd: Command) -> Result<AppStatus, Box<dyn Error>> {
        let event: Option<AppStatus> = match cmd {
            Command::Provide { path } => {
                let file = BinaryFile::new(&path);
                let serialized = file.make_typed().sign(self.network_client.get_keys()).unwrap();
                let obj_id = serialized.get_object_id();

                println!("Providing: {}", BASE64_STANDARD.encode(&obj_id));

                self.state.add(serialized).await;
                self.network_client.start_providing(obj_id).await;

                None
            }
            Command::Get { name } => {
                let id: ObjectId = BASE64_STANDARD.decode(&name)
                    .unwrap().try_into().unwrap();
                
                let providers = self.network_client.get_providers(id.clone()).await;
                if providers.is_empty() {
                    return Err(format!("Could not find provider for file {name}.").into());
                }

                let requests = providers.into_iter().map(|p| {
                    event!(Level::INFO, "Found provider: {p}");
                    let mut network_client = self.network_client.clone();
                    async move { network_client.request_file(p, id).await }.boxed()
                });

                let file_content = futures::future::select_ok(requests)
                    .await
                    .map_err(|_| "None of the providers returned file.")?
                    .0;

                let file = system::deserialize::<BinaryFile>(file_content.get_data());
                let path = dirs::download_dir().unwrap();
                println!("Saving {} to {}", file.filename, path.as_os_str().to_str().unwrap());
                file.save(&path);

                None
            }

            Command::Quit => {
                Some(AppStatus::Done)
            }
        };

        Ok(event.unwrap_or(AppStatus::Running))
    }
}

pub struct AppControllerHandle {
    tx: mpsc::Sender<Command>
}

impl AppControllerHandle {
    pub fn new(network_client: Client, network_events: EventStream) -> Self {
        let (tx, rx) = mpsc::channel(32);
        
        let mut mgr = AppController {
            state: AppStateHandle::new(),
            network_client,
            network_events,
            rx
        };

        spawn(async move { mgr.run().await; });

        Self { tx }
    }
    
    pub async fn send(&mut self, cmd: Command) {
        let _ = self.tx.send(cmd).await;
    }
}
