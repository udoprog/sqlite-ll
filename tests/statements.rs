#![allow(unused)]

use anyhow::Result;
use sqll::{OpenOptions, Pool, PoolError, SendStatement, Statements};

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

#[test]
fn test_pool() -> Result<()> {
    let mut c = OpenOptions::new();
    c.no_mutex().create();

    let temp = tempfile::TempDir::new()?;

    let _pool = Pool::<Read, Write>::new(c, temp.path().join("test.db"), 4)?;
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

    assert!(Pool::<NotReadOnly, Write>::new(c, temp.path().join("test.db"), 4).is_err());
    Ok(())
}
