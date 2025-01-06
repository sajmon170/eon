mod network;
mod event_loop;
mod stream_manager;

mod system;
mod parser;
mod minivault;
mod pins;
mod testing_obj;
mod core;

use std::{error::Error, io::Write, path::PathBuf, fs::File};

use clap::Parser;
use futures::{prelude::*, StreamExt};
use libp2p::{core::Multiaddr, multiaddr::Protocol, identity::{Keypair, self}};
use tokio::task::spawn;
use tracing_subscriber::EnvFilter;
use tracing_appender::{non_blocking, non_blocking::WorkerGuard};
use tracing::{Level, event};
use anyhow::Result;

fn init_tracing(name: &str) -> Result<WorkerGuard> {
    let file = File::create(format!("{name}.log"))?;
    let (non_blocking, guard) = non_blocking(file);

    let env_filter = EnvFilter::builder()
        .with_default_directive(Level::INFO.into())
        .from_env_lossy();

    tracing_subscriber::fmt()
        .with_writer(non_blocking)
        .with_env_filter(env_filter)
        .init();
    
    // Alternative subscriber - Tokio Console
    // console_subscriber::init();

    Ok(guard)
}

fn get_keypair(secret_key_seed: Option<u8>) -> Keypair {
    match secret_key_seed {
        Some(seed) => {
            let mut bytes = [0u8; 32];
            bytes[0] = seed;
            identity::Keypair::ed25519_from_bytes(bytes).unwrap()
        }
        None => identity::Keypair::generate_ed25519(),
    }
}


#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let opt = Opt::parse();

    let keypair = get_keypair(opt.secret_key_seed);
    let peer_id = keypair.public().to_peer_id();
    println!("My id: {peer_id}");

    let _guard = init_tracing(&peer_id.to_base58())?;

    event!(Level::INFO, "Hello.");

    let (mut network_client, mut network_events, network_event_loop) =
        network::new(keypair, opt.bootstrap_mode).await?;

    // Spawn the network task for it to run in the background.
    spawn(network_event_loop.run());

    // In case a listen address was provided use it, otherwise listen on any
    // address.
    match opt.listen_address {
        Some(addr) => network_client
            .start_listening(addr)
            .await
            .expect("Listening not to fail."),
        None => network_client
            .start_listening("/ip4/0.0.0.0/tcp/0".parse()?)
            .await
            .expect("Listening not to fail."),
    };

    // In case the user provided an address of a peer on the CLI, dial it.
    if let Some(addr) = opt.peer {
        let Some(Protocol::P2p(peer_id)) = addr.iter().last() else {
            return Err("Expect peer multiaddr to contain peer ID.".into());
        };
        network_client
            .dial(peer_id, addr)
            .await
            .expect("Dial to succeed");
    }

    //tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    if !opt.bootstrap_mode {
        network_client.bootstrap().await?;
        event!(Level::INFO, "Finished bootstrapping.");
    }

    match opt.argument {
        // Providing a file.
        CliArgument::Provide { path, name } => {
            // Advertise oneself as a provider of the file on the DHT.
            network_client.start_providing(name.clone()).await;

            loop {
                match network_events.next().await {
                    // Reply with the content of the file on incoming requests.
                    Some(event_loop::Event::InboundRequest { request, channel }) => {
                        event!(Level::INFO, "Responding to request.");
                        if request == name {
                            network_client
                                .respond_file(std::fs::read(&path)?, channel)
                                .await;
                        }
                    }
                    e => todo!("{:?}", e),
                }
            }
        }
        // Locating and getting a file.
        CliArgument::Get { name } => {
            // Locate all nodes providing the file.
            let providers = network_client.get_providers(name.clone()).await;
            if providers.is_empty() {
                return Err(format!("Could not find provider for file {name}.").into());
            }

            // Request the content of the file from each node.
            let requests = providers.into_iter().map(|p| {
                event!(Level::INFO, "Found provider: {p}");
                let mut network_client = network_client.clone();
                let name = name.clone();
                async move { network_client.request_file(p, name).await }.boxed()
            });

            // Await the requests, ignore the remaining once a single one succeeds.
            let file_content = futures::future::select_ok(requests)
                .await
                .map_err(|_| "None of the providers returned file.")?
                .0;

            std::io::stdout().write_all(&file_content)?;
        }
    }

    Ok(())
}

#[derive(Parser, Debug)]
#[clap(name = "libp2p file sharing example")]
struct Opt {
    /// Fixed value to generate deterministic peer ID.
    #[clap(long)]
    secret_key_seed: Option<u8>,

    #[clap(long)]
    peer: Option<Multiaddr>,

    #[clap(long)]
    listen_address: Option<Multiaddr>,

    #[clap(long, action)]
    bootstrap_mode: bool,

    #[clap(subcommand)]
    argument: CliArgument,
}

#[derive(Debug, Parser)]
enum CliArgument {
    Provide {
        #[clap(long)]
        path: PathBuf,
        #[clap(long)]
        name: String,
    },
    Get {
        #[clap(long)]
        name: String,
    },
}
