use crate::object_parser::*;
use crate::pins::{PinObject, Tag};
use crate::testing_obj::{Poem, BinaryFile};
use crate::core::*;
use tokio::{sync::{mpsc, oneshot}, task::spawn};

enum DataMessage {
    StoreObject(SignedObject),
    QueryObject(TypedObject, oneshot::Sender<Option<SignedObject>>)
}

struct AppState {
    parser: ObjectParser,
    rx: mpsc::Receiver<DataMessage>
}

impl AppState {
    async fn run(&mut self) {
        while let Some(request) = self.rx.recv().await {
            match request {
                DataMessage::QueryObject(query, sender) => self.get_object(id, sender),
                DataMessage::StoreObject(obj) => self.add_object(obj)
            }
        }
    }

    fn get_object(&self, id: ObjectId, sender: oneshot::Sender<Option<SignedObject>>) {
        let _ = sender.send(self.state.get_object(id));
    }

    fn add_object(&mut self, obj: SignedObject) {
        self.state.parse_object(obj);
    }
}

pub struct AppStateHandle {
    tx: mpsc::Sender<DataMessage>
}

impl AppStateHandle {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel(32);

        let mut state = SystemState::new();
        state.register_parser::<BinaryFile>();
        state.register_parser::<PinObject>();
        state.register_parser::<Poem>();
        state.register_parser::<Tag>();
        
        let mut state = AppState { state, rx };

        spawn(async move { state.run().await; });

        Self { tx }
    }

    pub async fn query(&mut self, query: TypedObject) -> Option<SignedObject> {
        let (tx, rx) = oneshot::channel();
        let _ = self.tx.send(DataMessage::QueryObject(query, tx)).await;
        rx.await.unwrap()
    }

    pub async fn add(&mut self, obj: SignedObject) {
        let _ = self.tx.send(DataMessage::StoreObject(obj)).await;
    }
}
