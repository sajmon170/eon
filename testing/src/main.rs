use libp2p_invert::event_subscriber;
use futures::channel::mpsc;
use libp2p::swarm::{NetworkBehaviour, SwarmEvent};
use libp2p::kad;
use libp2p::kad::QueryId;
use std::collections::HashMap;
use futures::StreamExt;
use futures::SinkExt;
use futures::channel::oneshot::Canceled;

/*
#[derive(Copy, Clone, Hash, Eq, PartialEq)]
struct QueryId;
*/

#[derive(Clone)]
pub(crate) struct Client {
    fn_sender: mpsc::Sender<EventLoopFn>
}

#[derive(NetworkBehaviour)]
pub(crate) struct Behaviour {
    pub kademlia: kad::Behaviour<kad::store::MemoryStore>,
}

#[event_subscriber(Behaviour)]
impl Client {
    async fn testing(&mut self) {
        let x = vec![1, 2, 3];
        let event_id = unsafe { std::mem::transmute::<usize, QueryId>(12) };
        let v = subscribe!(event_id: QueryId => SwarmEvent::Behaviour(BehaviourEvent::Kademlia(
            kad::Event::OutboundQueryProgressed {
                #[key] id,
                result:
                    kad::QueryResult::GetProviders(Ok(kad::GetProvidersOk::FoundProviders {
                        providers,
                        ..
                    })),
                ..
            },
        )));
    }

    async fn another_fun(&mut self) {
        let my_event = unsafe { std::mem::transmute::<usize, QueryId>(12) };
        let _ = subscribe!(my_event: QueryId => SwarmEvent::Behaviour(BehaviourEvent::Kademlia(
            kad::Event::OutboundQueryProgressed {
                #[key] id,
                result: kad::QueryResult::StartProviding(_),
                ..
            },
        )));
        println!("---");
    } 
}

fn main() {
    println!("Hello, world!");

    let (tx, _) = futures::channel::mpsc::channel(0);

    let mut client = Client { fn_sender: tx };
    client.testing();
    client.another_fun();
}
