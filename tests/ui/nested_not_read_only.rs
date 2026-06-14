use sqll::{SendStatement, Statements};

#[derive(Statements)]
struct Writer {
    #[sql = "CREATE TABLE foo (id INTEGER)"]
    create: SendStatement,
}

// A `read_only` collection must not be able to embed a collection that can
// mutate the database, otherwise its `IsReadOnly` implementation is unsound.
#[derive(Statements)]
#[sql(read_only)]
struct Reader {
    #[sql(statements)]
    nested: Writer,
}

fn main() {}
