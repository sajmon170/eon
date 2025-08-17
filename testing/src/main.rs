use libp2p_scaffolding::event_subscriber;

struct Test;

#[derive(Debug)]
struct QueryId;

#[event_subscriber]
impl Test {
    fn testing() {
        let x = vec![1, 2, 3];
        let event_id = QueryId;
        let v = subscribe!(event_id: QueryId => EventProgressed { #[key] id, data });
    }
}

fn main() {
    println!("Hello, world!");
    Test::testing();
}
