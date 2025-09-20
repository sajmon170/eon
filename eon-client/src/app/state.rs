use crate::net::network::KadPeerData;

use std::collections::HashMap;

use tokio::{sync::{mpsc, oneshot}, task::spawn};
use objects::prelude::*;

enum DataMessage {
    StoreObject(SignedObject),
    RpcObject(TypedObject, oneshot::Sender<Option<Vec<SignedObject>>>),
    StoreProviders(ObjectId, Vec<KadPeerData>),
    GetProviders(ObjectId, oneshot::Sender<Vec<KadPeerData>>)
}

struct AppState {
    parser: ObjectParser,
    known_providers: HashMap<ObjectId, Vec<KadPeerData>>,
    rx: mpsc::Receiver<DataMessage>
}

impl AppState {
    async fn run(&mut self) {
        while let Some(request) = self.rx.recv().await {
            match request {
                DataMessage::RpcObject(rpc, sender) => self.parse_rpc(rpc, sender),
                DataMessage::StoreObject(obj) => self.add_object(obj),
                DataMessage::StoreProviders(id, providers) => self.store_providers(id, providers),
                DataMessage::GetProviders(id, sender) => self.get_providers(id, sender)
            }
        }
    }

    fn parse_rpc(&mut self, rpc: TypedObject, sender: oneshot::Sender<Option<Vec<SignedObject>>>) {
        let _ = sender.send(self.parser.parse_rpc(rpc));
    }

    fn get_object(&self, id: ObjectId, sender: oneshot::Sender<Option<SignedObject>>) {
        let _ = sender.send(self.parser.get_object(id));
    }

    fn add_object(&mut self, obj: SignedObject) {
        self.parser.parse_object(obj);
    }

    fn store_providers(&mut self, id: ObjectId, providers: Vec<KadPeerData>) {
        self.known_providers.insert(id, providers);
    }

    fn get_providers(&self, id: ObjectId, sender: oneshot::Sender<Vec<KadPeerData>>) {
        let providers = self.known_providers.get(&id).cloned().unwrap_or_default();
        let _ = sender.send(providers);
    }
}

pub struct AppStateHandle {
    tx: mpsc::Sender<DataMessage>
}

impl AppStateHandle {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel(32);

        let mut parser = ObjectParser::new();

        parser.register_object_parser::<BinaryFile>();
        parser.register_object_parser::<PinObject>();
        parser.register_object_parser::<Poem>();
        parser.register_object_parser::<Tag>();
        parser.register_rpc_parser::<StoreObject>();
        parser.register_rpc_parser::<DeleteObject>();
        parser.register_rpc_parser::<GetObject>();
        parser.register_query_parser::<CompositeQuery>();
        parser.register_query_parser::<WithUuid>();
        parser.register_query_parser::<PinnedWith>();

        let known_providers = HashMap::new();

        let mut state = AppState { parser, known_providers, rx };

        spawn(async move {
            state.run().await;
        });

        Self { tx }
    }

    pub async fn rpc(&mut self, rpc: TypedObject) -> Option<Vec<SignedObject>> {
        let (tx, rx) = oneshot::channel();
        let _ = self.tx.send(DataMessage::RpcObject(rpc, tx)).await;
        rx.await.unwrap()
    }

    pub async fn add(&mut self, obj: SignedObject) {
        let _ = self.tx.send(DataMessage::StoreObject(obj)).await;
    }

    pub async fn store_providers(&mut self, id: ObjectId, providers: Vec<KadPeerData>) {
        let _ = self.tx.send(DataMessage::StoreProviders(id, providers)).await;
    }

    pub async fn get_providers(&self, id: ObjectId) -> Vec<KadPeerData> {
        let (tx, rx) = oneshot::channel();
        let _ = self.tx.send(DataMessage::GetProviders(id, tx)).await;
        rx.await.unwrap()
    }
}
