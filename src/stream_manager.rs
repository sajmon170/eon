use libp2p_stream::{Control, IncomingStreams};
use libp2p::StreamProtocol;
use libp2p::PeerId;
use tokio::sync::mpsc;
use base64::prelude::*;
use anyhow::{anyhow, Result};
use tokio_util::compat::FuturesAsyncReadCompatExt;
use futures::StreamExt;
use tokio::sync::oneshot;
use std::collections::{HashMap, HashSet, VecDeque};

type Hash = [u8; 32];
type StreamSender = oneshot::Sender<libp2p::Stream>;

enum StreamCommand {
    Listen(PeerId, StreamSender),
    OpenStream(PeerId, StreamSender)
}

pub struct StreamRouterHandle {
    tx: mpsc::Sender<(Hash, StreamCommand)>
}

impl StreamRouterHandle {
    pub fn new(control: Control) -> Self {
        let (tx, rx) = mpsc::channel(32);
        
        let mut router = StreamRouter {
            control,
            handlers: HashMap::new(),
            rx
        };

        tokio::spawn(async move {
            router.run().await;
        });

        Self { tx }
    }

    pub async fn open_stream(&mut self, hash: Hash, id: PeerId) -> Result<libp2p::Stream> {
        let (tx, rx) = oneshot::channel();
        let _ = self.tx.send((hash, StreamCommand::OpenStream(id, tx)));
        rx.await.map_err(|e| anyhow!(e))
    }

    pub async fn listen_stream(&mut self, hash: Hash, id: PeerId) -> Result<libp2p::Stream> {
        let (tx, rx) = oneshot::channel();
        let _ = self.tx.send((hash, StreamCommand::Listen(id, tx)));
        rx.await.map_err(|e| anyhow!(e))
    }
}

struct StreamRouter {
    control: Control,
    handlers: HashMap<Hash, StreamProtocolActor>,
    rx: mpsc::Receiver<(Hash, StreamCommand)>
}

impl StreamRouter {
    async fn run(&mut self) {
        while let Some((hash, cmd)) = self.rx.recv().await {
            self.handlers
                .entry(hash)
                .or_insert(StreamProtocolActor::new(&mut self.control, &hash))
                .send(cmd).await;
        }
    }
}


struct StreamProtocolActor {
    tx: mpsc::Sender<StreamCommand>
}

impl StreamProtocolActor {
    fn new(control: &mut Control, hash: &Hash) -> Self {
        let (tx, rx) = mpsc::channel(32);
        let mut handler = StreamProtocolHandler::new(control, hash, rx);

        tokio::spawn(async move {
            handler.run().await.unwrap();
        });

        Self { tx }
    }

    async fn send(&mut self, cmd: StreamCommand) {
        let _ = self.tx.send(cmd).await;
    }
}

struct StreamProtocolHandler {
    control: Control,
    protocol: StreamProtocol,
    peers: HashMap<PeerId, StreamSender>,
    rx: mpsc::Receiver<StreamCommand>
}

impl StreamProtocolHandler {
    fn get_stream_protocol(hash: &Hash) -> StreamProtocol {
        let proto_str = format!("/file-encode/{}", BASE64_STANDARD.encode(hash));
        StreamProtocol::try_from_owned(proto_str).unwrap()
    }

    fn new(control: &mut Control, hash: &Hash, rx: mpsc::Receiver<StreamCommand>) -> Self {
        Self {
            control: control.clone(),
            protocol: Self::get_stream_protocol(hash),
            peers: HashMap::new(),
            rx
        }
    }

    async fn run(&mut self) -> Result<()> {
        let mut listener = self.control.accept(self.protocol.clone()).unwrap();
        
        loop {
            tokio::select! {
                Some((id, stream)) = listener.next() => {
                    if let Some(listener) = self.peers.remove(&id) {
                        let _ = listener.send(stream);
                    }
                }
                Some(cmd) = self.rx.recv() => self.handle_command(cmd).await?,
                else => break
            }
        }

        Ok(())
    }

    async fn open_stream(&mut self, id: PeerId, sender: StreamSender) -> Result<()> {
        let stream = self.control.open_stream(id, self.protocol.clone()).await?;
        let _ = sender.send(stream);

        Ok(())
    }

    async fn handle_command(&mut self, cmd: StreamCommand) -> Result<()> {
        match cmd {
            StreamCommand::Listen(id, sender) => {
                self.peers.insert(id, sender);
            },
            StreamCommand::OpenStream(id, sender) => {
                self.open_stream(id, sender).await?;
            }
        };

        Ok(())
    }
}
/*

struct StreamManager {
    control: Control,
    commands: mpsc::Receiver<StreamCommand>
}

impl StreamManager {
    fn get_stream_protocol(hash: &Hash) -> StreamProtocol {
        let proto_str = format!("/file-encode/{}", BASE64_STANDARD.encode(hash));
        StreamProtocol::try_from_owned(proto_str).unwrap()
    }
    
    async fn request_hash(mut ctrl: Control, peer: PeerId, hash: Hash) -> Result<()> {
        let stream = ctrl.open_stream(peer, Self::get_stream_protocol(&hash)).await?;

        let mut bytes: &[u8] = b"Hello, world!";

        // TODO: replace this with vault
        tokio::io::copy(&mut bytes, &mut stream.compat()).await?;

        Ok(())
    }

    async fn await_hash(mut ctrl: Control, peer: PeerId, hash: Hash) -> Result<()> {
        // TODO: test whether this creates a race condition when multiple peers
        // ask for the same file

        let (id, mut stream) = ctrl.accept(Self::get_stream_protocol(&hash))?
            .next().await
            .ok_or(anyhow!("Stream dropped prematurely"))?;

        /*
        if id != peer {
            Err(anyhow!("This "))
    }
        */

        let mut bytes = Vec::with_capacity(1000);

        // TODO: replace this with vault
        tokio::io::copy(&mut stream.compat(), &mut bytes).await?;

        println!("{}", std::str::from_utf8(&bytes).unwrap());
        
        Ok(())
    }

    async fn run(&mut self) {
        while let Some(cmd) = self.commands.recv().await {
            match cmd {
                StreamCommand::Request(id, hash)
                    => tokio::spawn(Self::request_hash(self.control.clone(), id, hash)),
                StreamCommand::Await(id, hash)
                    => tokio::spawn(Self::await_hash(self.control.clone(), id, hash))
            };
        }
    }
}

struct StreamManagerHandle {
    
}
*/
