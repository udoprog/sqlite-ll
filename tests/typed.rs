//! Integration tests for the typed statement API.
//!
//! Behavioural examples for the `BoundStatement` methods (`first`, `next`,
//! `reset`, and `Drop`) live as doctests on the methods themselves. This file
//! covers the end-to-end derive flow plus the `TryFrom` conversion error paths,
//! which assert exact diagnostics and are awkward to express as doctests.

use anyhow::Result;
use sqll::{OpenOptions, Statements, TryFromSendStatementError, TypedStatement};

#[derive(Statements)]
#[sql(read_only)]
struct Read {
    #[sql = "SELECT name, age FROM users ORDER BY age"]
    all: TypedStatement<(), (String, i32)>,
}

#[derive(Statements)]
struct Write {
    #[sql(statements)]
    read: Read,
    #[sql = "INSERT INTO users (name, age) VALUES (?, ?)"]
    insert: TypedStatement<(String, i32), ()>,
}

fn setup() -> Result<sqll::Connection> {
    let c = OpenOptions::new()
        .no_mutex()
        .read_write()
        .create()
        .open_in_memory()?;

    c.execute("CREATE TABLE users (name TEXT, age INTEGER)")?;
    Ok(c)
}

#[test]
fn end_to_end_execute_and_query() -> Result<()> {
    let mut c = setup()?;
    let mut w = Write::build(&mut c)?;

    w.insert.execute(("Alice".to_string(), 25))?;
    w.insert.execute(("Bob".to_string(), 35))?;
    w.insert.execute(("Charlie".to_string(), 30))?;

    let mut out = Vec::new();
    let mut stmt = w.read.all.query()?;
    while let Some(row) = stmt.next()? {
        out.push(row);
    }

    assert_eq!(
        out,
        vec![
            ("Alice".to_string(), 25),
            ("Charlie".to_string(), 30),
            ("Bob".to_string(), 35),
        ]
    );
    Ok(())
}

/// Prepare a parameterless single-column statement and coerce it into a
/// thread-safe [`sqll::SendStatement`] for the `TryFrom` conversion tests.
fn select_one_send(c: &sqll::Connection) -> Result<sqll::SendStatement> {
    let s = c.prepare("SELECT 1")?;
    // Safe: the connection was opened with `no_mutex`.
    Ok(unsafe { s.into_send()? })
}

#[test]
fn try_from_wrong_bind_count() -> Result<()> {
    let c = setup()?;
    let send = select_one_send(&c)?;

    // "SELECT 1" binds 0 parameters, but the target type expects 1.
    // `map(drop)` discards the non-`Debug` `Ok` value so `unwrap_err` compiles.
    let err = TypedStatement::<(i32,), (i32,)>::try_from(send)
        .map(drop)
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "unexpected bind parameter count for statement: expected 1, got 0"
    );
    // Assert the error is publicly nameable and a real `std::error::Error`.
    fn assert_error<E: core::error::Error>(_: &E) {}
    assert_error::<TryFromSendStatementError>(&err);
    Ok(())
}

#[test]
fn try_from_wrong_column_count() -> Result<()> {
    let c = setup()?;
    let send = select_one_send(&c)?;

    // "SELECT 1" yields 1 column, but the target type expects 2.
    let err = TypedStatement::<(), (i32, i32)>::try_from(send)
        .map(drop)
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "unexpected column count for statement: expected 2, got 1"
    );
    Ok(())
}

#[test]
fn try_from_matching_counts() -> Result<()> {
    let c = setup()?;
    let send = select_one_send(&c)?;

    let mut stmt = TypedStatement::<(), (i32,)>::try_from(send).unwrap();
    assert_eq!(stmt.query()?.first()?, Some((1,)));
    Ok(())
}
