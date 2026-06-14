use sqll::{SendStatement, Statements};

#[derive(Statements)]
struct UnknownKey {
    #[sql(nope)]
    field: SendStatement,
}

fn main() {}
