#![allow(dead_code, unused)]
mod app;
mod net;

use std::{error::Error, io::Write, path::PathBuf, fs::File};

use clap::Parser;
use futures::{prelude::*, StreamExt};
use libp2p::{core::Multiaddr, multiaddr::Protocol, identity::{Keypair, self}};
use tokio::task::spawn;
use tracing_subscriber::EnvFilter;
use tracing_appender::{non_blocking, non_blocking::WorkerGuard};
use tracing::{Level, event};
use anyhow::Result;

use crate::{net::network, app::cli::AppCli};

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
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let opt = Opt::parse();

    let keypair = get_keypair(opt.secret_key_seed);
    let peer_id = keypair.public().to_peer_id();
    println!("My id: {peer_id}");

    let _guard = init_tracing(&peer_id.to_base58())?;

    event!(Level::INFO, "Hello.");

    let network_client =
        network::new(keypair, opt.bootstrap_mode).await?;

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

    if !opt.bootstrap_mode {
        network_client.bootstrap().await?;
        event!(Level::INFO, "Finished bootstrapping.");
    }

    let mut app = AppCli::new(network_client);
    app.run().await?;

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
pub enum CliArgument {
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
