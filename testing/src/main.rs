use libp2p_scaffolding::event_subscriber;

struct Test;

#[derive(Debug)]
struct QueryId;

#[event_subscriber]
impl Test {
    fn testing() {
        let x = vec![1, 2, 3];
        let event_id = QueryId;
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

    fn another_fun(&self) {
        let my_event = QueryId;
        let _ = subscribe!(my_event: QueryId => SwarmEvent::Other(gossipsub::Event{#[key] id, result}));
        println!("---");
    } 

    fn something_other(&self) {
        println!("--");
        let something = QueryId;
        let _ = subscribe!(something: QueryId => SwarmEvent::Behaviour(something::Fun { other, #[key] val}));
    }
}

fn main() {
    println!("Hello, world!");
    Test::testing();

    let test = Test;
    test.another_fun();
    test.something_other();
}
