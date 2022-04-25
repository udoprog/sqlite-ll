# sqlite-ll

[<img alt="github" src="https://img.shields.io/badge/github-udoprog/sqlite-ll?style=for-the-badge&logo=github" height="20">](https://github.com/udoprog/sqlite-ll)
[<img alt="crates.io" src="https://img.shields.io/crates/v/sqlite-ll.svg?style=for-the-badge&color=fc8d62&logo=rust" height="20">](https://crates.io/crates/sqlite-ll)
[<img alt="docs.rs"
src="https://img.shields.io/badge/docs.rs-sqlite-ll?style=for-the-badge&logoColor=white&logo=data:image/svg+xml;base64,PHN2ZyByb2xlPSJpbWciIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyIgdmlld0JveD0iMCAwIDUxMiA1MTIiPjxwYXRoIGZpbGw9IiNmNWY1ZjUiIGQ9Ik00ODguNiAyNTAuMkwzOTIgMjE0VjEwNS41YzAtMTUtOS4zLTI4LjQtMjMuNC0zMy43bC0xMDAtMzcuNWMtOC4xLTMuMS0xNy4xLTMuMS0yNS4zIDBsLTEwMCAzNy41Yy0xNC4xIDUuMy0yMy40IDE4LjctMjMuNCAzMy43VjIxNGwtOTYuNiAzNi4yQzkuMyAyNTUuNSAwIDI2OC45IDAgMjgzLjlWMzk0YzAgMTMuNiA3LjcgMjYuMSAxOS45IDMyLjJsMTAwIDUwYzEwLjEgNS4xIDIyLjEgNS4xIDMyLjIgMGwxMDMuOS01MiAxMDMuOSA1MmMxMC4xIDUuMSAyMi4xIDUuMSAzMi4yIDBsMTAwLTUwYzEyLjItNi4xIDE5LjktMTguNiAxOS45LTMyLjJWMjgzLjljMC0xNS05LjMtMjguNC0yMy40LTMzLjd6TTM1OCAyMTQuOGwtODUgMzEuOXYtNjguMmw4NS0zN3Y3My4zek0xNTQgMTA0LjFsMTAyLTM4LjIgMTAyIDM4LjJ2LjZsLTEwMiA0MS40LTEwMi00MS40di0uNnptODQgMjkxLjFsLTg1IDQyLjV2LTc5LjFsODUtMzguOHY3NS40em0wLTExMmwtMTAyIDQxLjQtMTAyLTQxLjR2LS42bDEwMi0zOC4yIDEwMiAzOC4ydi42em0yNDAgMTEybC04NSA0Mi41di03OS4xbDg1LTM4Ljh2NzUuNHptMC0xMTJsLTEwMiA0MS40LTEwMi00MS40di0uNmwxMDItMzguMiAxMDIgMzguMnYuNnoiPjwvcGF0aD48L3N2Zz4K"
height="20">](https://docs.rs/sqlite-ll) [<img alt="build status"
src="https://img.shields.io/github/workflow/status/udoprog/sqlite-ll/CI/main?style=for-the-badge"
height="20">](https://github.com/udoprog/sqlite-ll/actions?query=branch%3Amain)

Low-level interface to the [SQLite] database.

This is a rewrite of the [sqlite crate], and components used from there have
been copied under the MIT license.

### Why do we need a low-level interface?

It is difficult to use prepared statements with existing crates, because
they are all implemented in a manner which requires the caller to borrow the
connection in use.

Prepared statements can be expensive to create and *should* be cached and
re-used to achieve the best performance.

The way this crate gets around this is by making the `prepare` function
`unsafe`, so the impetus is on the caller to ensure that the connection it's
related to stays alive for the duration of the prepared statement.

### Example

Open a connection, create a table, and insert some rows:

```rust
let c = sqlite_ll::Connection::open(":memory:")?;

c.execute(
    "
    CREATE TABLE users (name TEXT, age INTEGER);
    INSERT INTO users VALUES ('Alice', 42);
    INSERT INTO users VALUES ('Bob', 69);
    ",
)?;
```

Select some rows and process them one by one as plain text:

```rust
c.iterate("SELECT * FROM users WHERE age > 50", |pairs| {
    for &(column, value) in pairs.iter() {
        println!("{} = {}", column, value.unwrap());
    }

    true
})?;
```

The same query using a prepared statement, which is much more efficient than
parsing and running statements ad-hoc. They must be reset before every
re-use.

```rust
use sqlite_ll::State;
let mut statement = connection.prepare("SELECT * FROM users WHERE age > ?")?;

let mut results = Vec::new();

for age in [40, 50] {
    statement.reset()?;
    statement.bind(1, age)?;

    while let State::Row = statement.next()? {
        results.push((statement.read::<String>(0)?, statement.read::<i64>(1)?));
    }
}

let expected = vec![
    (String::from("Alice"), 42),
    (String::from("Bob"), 69),
    (String::from("Bob"), 69),
];

assert_eq!(results, expected);
```

[sqlite crate]: https://github.com/stainless-steel/sqlite
[SQLite]: https://www.sqlite.org

License: Apache-2.0/MIT
