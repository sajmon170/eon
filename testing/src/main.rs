#![allow(dead_code, unused)]

use libp2p_invert::{event_subscriber, swarm_client};
use libp2p::swarm::{NetworkBehaviour, SwarmEvent};
use libp2p::kad;
use libp2p::kad::QueryId;
use std::collections::HashMap;

/*
#[derive(Copy, Clone, Hash, Eq, PartialEq)]
struct QueryId;
*/

#[derive(NetworkBehaviour)]
pub(crate) struct Behaviour {
    pub kademlia: kad::Behaviour<kad::store::MemoryStore>,
}

#[swarm_client(Behaviour)]
#[derive(Clone)]
pub(crate) struct Client;

#[event_subscriber(Behaviour)]
impl Client {
    async fn testing(&self) {
        let x = vec![1, 2, 3];
        let event_id = unsafe { std::mem::transmute::<usize, QueryId>(12) };
        let result = subscribe!(event_id: QueryId => SwarmEvent::Behaviour(BehaviourEvent::Kademlia(
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

    async fn another_fun(&self) {
        let my_event = unsafe { std::mem::transmute::<usize, QueryId>(12) };
        let _ = subscribe!(my_event: QueryId => SwarmEvent::Behaviour(BehaviourEvent::Kademlia(
            kad::Event::OutboundQueryProgressed {
                #[key] id,
                result: kad::QueryResult::StartProviding(_),
                ..
            },
        ))).await;
        println!("---");
    } 

    async fn yet_another_fun(&self) {
        let my_event = unsafe { std::mem::transmute::<usize, QueryId>(12) };
        let id = subscribe!(_ => SwarmEvent::Behaviour(BehaviourEvent::Kademlia(
            kad::Event::OutboundQueryProgressed {
                id,
                result: kad::QueryResult::StartProviding(_),
                ..
            },
        ))).await;
        println!("---");
    } 
}

fn main() {
    println!("Hello, world!");

    let (tx, _) = tokio::sync::mpsc::channel(0);

    let client = Client { fn_sender: tx };
    client.testing();
    client.another_fun();
    client.yet_another_fun();
}
