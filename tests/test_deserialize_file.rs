use std::fs;

use anyhow::Result;
use sqll::{Connection, OwnedBytes};
use tempfile::tempdir;

#[test]
fn deserialize_file() -> Result<()> {
    let dir = tempdir()?;
    let path = dir.path().join("database.sqlite");

    {
        let c = Connection::open(&path)?;

        c.execute(
            r#"
			CREATE TABLE users (name TEXT, age INTEGER);
			INSERT INTO users VALUES ('Alice', 42);
			INSERT INTO users VALUES ('Bob', 72);
			"#,
        )?;
    }

    let bytes = fs::read(path)?;

    let mut data = OwnedBytes::new();
    data.extend_from_slice(&bytes)?;
    assert!(!data.is_empty());

    let c = Connection::open_in_memory()?;
    c.deserialize(c"main", data)?;

    let rows = c
        .prepare("SELECT name, age FROM users ORDER BY name")?
        .iter::<(String, u32)>()
        .collect::<sqll::Result<Vec<_>>>()?;

    assert_eq!(rows, [("Alice".to_string(), 42), ("Bob".to_string(), 72)]);
    Ok(())
}
