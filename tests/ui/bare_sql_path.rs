use sqll::{SendStatement, Statements};

#[derive(Statements)]
struct BareSqlPath {
    #[sql]
    field: SendStatement,
}

fn main() {}
