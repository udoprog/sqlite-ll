#![allow(unused)]

use anyhow::Result;
use sqll::{OpenOptions, PoolBuilder, PoolError, SendStatement, Statements};

#[derive(Statements)]
struct TupleStatements(
    #[sql = "SELECT 1"] SendStatement,
    #[sql = "CREATE TABLE foo (id INTEGER)"] SendStatement,
);

#[derive(Statements)]
#[sql(read_only)]
struct TupleReadOnlyStatements(#[sql = "SELECT 1"] SendStatement);

#[derive(Statements)]
struct Write {
    #[sql = "SELECT 1"]
    select_one: SendStatement,
    #[sql = "CREATE TABLE foo (id INTEGER)"]
    create_table: SendStatement,
}

#[derive(Statements)]
#[sql(read_only)]
struct Read {
    #[sql = "SELECT 1"]
    select_one: SendStatement,
}

#[derive(Statements)]
struct WriteWithRead {
    #[sql(statements)]
    read: Read,
    #[sql = "CREATE TABLE foo (id INTEGER)"]
    create_table: SendStatement,
}

// A read-only collection may embed another read-only collection.
#[derive(Statements)]
#[sql(read_only)]
struct ReadWithRead {
    #[sql(statements)]
    inner: Read,
    #[sql = "SELECT 2"]
    select_two: SendStatement,
}

#[test]
fn test_pool() -> Result<()> {
    let mut c = OpenOptions::new();
    c.no_mutex().create();

    let temp = tempfile::TempDir::new()?;

    let _pool = PoolBuilder::new(c, 4).open::<Read, Write>(temp.path().join("test.db"))?;
    Ok(())
}

#[test]
fn test_nested_statements() -> Result<()> {
    let mut c = OpenOptions::new();
    c.no_mutex().create();

    let temp = tempfile::TempDir::new()?;

    // Build a pool whose read and write sides both contain nested statement
    // collections, exercising the recursive `Statements::build`.
    let _pool =
        PoolBuilder::new(c, 4).open::<ReadWithRead, WriteWithRead>(temp.path().join("test.db"))?;
    Ok(())
}

#[derive(Statements)]
#[sql(read_only)]
struct NotReadOnly {
    #[sql = "SELECT 1"]
    select_one: SendStatement,
    #[sql = "CREATE TABLE foo (id INTEGER)"]
    create_table: SendStatement,
}

#[test]
fn test_not_read_only() -> Result<()> {
    let mut c = OpenOptions::new();
    c.no_mutex().create();

    let temp = tempfile::TempDir::new()?;

    assert!(
        PoolBuilder::new(c, 4)
            .open::<NotReadOnly, Write>(temp.path().join("test.db"))
            .is_err()
    );
    Ok(())
}

// Statements that reference a table which only exists if the connection setup
// created it.
#[derive(Statements)]
#[sql(read_only)]
struct ReadItems {
    #[sql = "SELECT id FROM items"]
    select: SendStatement,
}

#[derive(Statements)]
struct WriteItems {
    #[sql = "INSERT INTO items (id) VALUES (?)"]
    insert: SendStatement,
}

#[test]
fn test_setup_creates_schema() -> Result<()> {
    let mut options = OpenOptions::new();
    options.no_mutex().create();

    let temp = tempfile::TempDir::new()?;

    // Without any setup, preparing `SELECT id FROM items` fails because the
    // table does not exist yet.
    assert!(
        PoolBuilder::new(options, 2)
            .open::<ReadItems, WriteItems>(temp.path().join("missing.db"))
            .is_err()
    );

    // The write setup runs on the write connection before any statement is
    // prepared and before the read connections are opened, so creating the
    // table there makes both the write and read statements preparable. This
    // also guards against the read/write setup being applied to the wrong
    // connection.
    let _pool = PoolBuilder::new(options, 2)
        .with_write_setup(|c| {
            c.execute("CREATE TABLE items (id INTEGER)")?;
            Ok(())
        })
        .open::<ReadItems, WriteItems>(temp.path().join("created.db"))?;
    Ok(())
}

#[test]
fn test_setup_error_surfaces() -> Result<()> {
    let mut options = OpenOptions::new();
    options.no_mutex().create();

    let temp = tempfile::TempDir::new()?;

    // An error returned from the setup closure aborts pool construction.
    let result = PoolBuilder::new(options, 1)
        .with_write_setup(|c| {
            c.execute("THIS IS NOT VALID SQL")?;
            Ok(())
        })
        .open::<Read, Write>(temp.path().join("test.db"));

    assert!(result.is_err());
    Ok(())
}
