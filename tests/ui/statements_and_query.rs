use sqll::{SendStatement, Statements};

#[derive(Statements)]
#[sql(read_only)]
struct Inner {
    #[sql = "SELECT 1"]
    one: SendStatement,
}

#[derive(Statements)]
struct BothStatementsAndQuery {
    #[sql(statements)]
    #[sql = "SELECT 1"]
    field: Inner,
}

fn main() {}
