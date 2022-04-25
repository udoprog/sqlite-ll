use sqlite_ll::{Code, Connection, OpenOptions, State, Type, Value};
use std::{path::Path, thread};
use temporary::Directory;

// Test cases copied from https://github.com/stainless-steel/sqlite under the
// MIT license.

#[test]
fn connection_change_count() -> sqlite_ll::Result<()> {
    let c = setup_users(":memory:")?;
    assert_eq!(c.change_count(), 1);
    assert_eq!(c.total_change_count(), 1);

    c.execute("INSERT INTO users VALUES (2, 'Bob', NULL, NULL, NULL)")?;
    assert_eq!(c.change_count(), 1);
    assert_eq!(c.total_change_count(), 2);

    c.execute("UPDATE users SET name = 'Bob' WHERE id = 1")?;
    assert_eq!(c.change_count(), 1);
    assert_eq!(c.total_change_count(), 3);

    c.execute("DELETE FROM users")?;
    assert_eq!(c.change_count(), 2);
    assert_eq!(c.total_change_count(), 5);
    Ok(())
}

#[test]
fn connection_error() -> sqlite_ll::Result<()> {
    let connection = setup_users(":memory:")?;
    let e = connection.execute(":)").unwrap_err();
    assert_eq!(e.code(), Code::ERROR);
    Ok(())
}

#[test]
fn connection_iterate() -> sqlite_ll::Result<()> {
    macro_rules! pair(
        ($one:expr, $two:expr) => (($one, Some($two)));
    );

    let connection = setup_users(":memory:")?;

    let mut done = false;
    let statement = "SELECT * FROM users";
    connection.iterate(statement, |pairs| {
        assert_eq!(pairs.len(), 5);
        assert_eq!(pairs[0], pair!("id", "1"));
        assert_eq!(pairs[1], pair!("name", "Alice"));
        assert_eq!(pairs[2], pair!("age", "42.69"));
        assert_eq!(pairs[3], pair!("photo", "\x42\x69"));
        assert_eq!(pairs[4], ("email", None));
        done = true;
        true
    })?;
    assert!(done);
    Ok(())
}

#[test]
fn connection_open_with_flags() -> Result<(), Box<dyn std::error::Error>> {
    let directory = Directory::new("sqlite")?;
    let path = directory.path().join("database.sqlite3");

    setup_users(&path)?;

    let flags = OpenOptions::new().set_read_only();
    let connection = flags.open(path)?;
    let e = connection
        .execute("INSERT INTO users VALUES (2, 'Bob', NULL, NULL, NULL)")
        .unwrap_err();

    assert_eq!(e.code(), Code::READONLY);
    Ok(())
}

#[test]
fn connection_set_busy_handler() -> Result<(), Box<dyn std::error::Error>> {
    let directory = Directory::new("sqlite")?;
    let path = directory.path().join("database.sqlite3");
    setup_users(&path)?;

    let guards = (0..100)
        .map(|_| {
            let path = path.to_path_buf();
            thread::spawn(move || {
                let mut connection = Connection::open(&path)?;
                connection.set_busy_handler(|_| true)?;
                let statement = "INSERT INTO users VALUES (?, ?, ?, ?, ?)";
                let mut statement = connection.prepare(statement)?;
                statement.bind(1, 2i64)?;
                statement.bind(2, "Bob")?;
                statement.bind(3, 69.42)?;
                statement.bind(4, &[0x69u8, 0x42u8][..])?;
                statement.bind(5, ())?;
                assert_eq!(statement.step()?, State::Done);
                Ok::<_, sqlite_ll::Error>(true)
            })
        })
        .collect::<Vec<_>>();

    for guard in guards {
        assert!(guard.join().unwrap()?);
    }

    Ok(())
}

#[test]
fn statement_bind() -> sqlite_ll::Result<()> {
    let c = setup_users(":memory:")?;
    let statement = "INSERT INTO users VALUES (?, ?, ?, ?, ?)";
    let mut s = c.prepare(statement)?;

    s.bind(1, 2i64)?;
    s.bind(2, "Bob")?;
    s.bind(3, 69.42)?;
    s.bind(4, &[0x69u8, 0x42u8][..])?;
    s.bind(5, ())?;
    assert_eq!(s.step()?, State::Done);
    Ok(())
}

#[test]
fn statement_bind_with_nullable() -> sqlite_ll::Result<()> {
    let connection = setup_users(":memory:")?;
    let s = "INSERT INTO users VALUES (?, ?, ?, ?, ?)";
    let mut s = connection.prepare(s)?;

    s.bind(1, None::<i64>)?;
    s.bind(2, None::<&str>)?;
    s.bind(3, None::<f64>)?;
    s.bind(4, None::<&[u8]>)?;
    s.bind(5, None::<&str>)?;
    assert_eq!(s.step()?, State::Done);

    let s = "INSERT INTO users VALUES (?, ?, ?, ?, ?)";
    let mut s = connection.prepare(s)?;

    s.bind(1, Some(2i64))?;
    s.bind(2, Some("Bob"))?;
    s.bind(3, Some(69.42))?;
    s.bind(4, Some(&[0x69u8, 0x42u8][..]))?;
    s.bind(5, None::<&str>)?;
    assert_eq!(s.step()?, State::Done);
    Ok(())
}

#[test]
fn statement_bind_by_name() -> sqlite_ll::Result<()> {
    let connection = setup_users(":memory:")?;
    let s = "INSERT INTO users VALUES (:id, :name, :age, :photo, :email)";
    let mut s = connection.prepare(s)?;

    s.bind_by_name(":id", 2i64)?;
    s.bind_by_name(":name", "Bob")?;
    s.bind_by_name(":age", 69.42)?;
    s.bind_by_name(":photo", &[0x69u8, 0x42u8][..])?;
    s.bind_by_name(":email", ())?;
    assert!(s.bind_by_name(":missing", 404).is_err());
    assert_eq!(s.step()?, State::Done);
    Ok(())
}

#[test]
fn statement_column_count() -> sqlite_ll::Result<()> {
    let connection = setup_users(":memory:")?;
    let s = "SELECT * FROM users";
    let mut s = connection.prepare(s)?;

    assert_eq!(s.step()?, State::Row);

    assert_eq!(s.column_count(), 5);
    Ok(())
}

#[test]
fn statement_column_name() -> sqlite_ll::Result<()> {
    let connection = setup_users(":memory:")?;
    let s = "SELECT id, name, age, photo AS user_photo FROM users";
    let s = connection.prepare(s)?;

    let names = s.column_names()?;
    assert_eq!(names, vec!["id", "name", "age", "user_photo"]);
    assert_eq!("user_photo", s.column_name(3)?);
    Ok(())
}

#[test]
fn statement_column_type() -> sqlite_ll::Result<()> {
    let connection = setup_users(":memory:")?;
    let s = "SELECT * FROM users";
    let mut s = connection.prepare(s)?;

    assert_eq!(s.column_type(0), Type::Null);
    assert_eq!(s.column_type(1), Type::Null);
    assert_eq!(s.column_type(2), Type::Null);
    assert_eq!(s.column_type(3), Type::Null);

    assert_eq!(s.step()?, State::Row);

    assert_eq!(s.column_type(0), Type::Integer);
    assert_eq!(s.column_type(1), Type::Text);
    assert_eq!(s.column_type(2), Type::Float);
    assert_eq!(s.column_type(3), Type::Blob);
    Ok(())
}

#[test]
fn statement_parameter_index() -> sqlite_ll::Result<()> {
    let connection = setup_users(":memory:")?;
    let statement = "INSERT INTO users VALUES (:id, :name, :age, :photo, :email)";
    let mut statement = connection.prepare(statement)?;

    statement.bind(statement.parameter_index(":id")?.unwrap(), 2)?;
    statement.bind(statement.parameter_index(":name")?.unwrap().into(), "Bob")?;
    statement.bind(statement.parameter_index(":age")?.unwrap().into(), 69.42)?;
    statement.bind(
        statement.parameter_index(":photo")?.unwrap().into(),
        &[0x69u8, 0x42u8][..],
    )?;
    statement.bind(statement.parameter_index(":email")?.unwrap().into(), ())?;
    assert_eq!(statement.parameter_index(":missing")?, None);
    assert_eq!(statement.step()?, State::Done);
    Ok(())
}

#[test]
fn statement_read() -> sqlite_ll::Result<()> {
    let c = setup_users(":memory:")?;
    let s = "SELECT * FROM users";
    let mut s = c.prepare(s)?;

    assert_eq!(s.step()?, State::Row);
    assert_eq!(s.read::<i64>(0)?, 1);
    assert_eq!(s.read::<String>(1)?, String::from("Alice"));
    assert_eq!(s.read::<f64>(2)?, 42.69);
    assert_eq!(s.read::<Vec<u8>>(3)?, vec![0x42, 0x69]);
    assert_eq!(s.read::<Value>(4)?, Value::Null);
    assert_eq!(s.step()?, State::Done);
    Ok(())
}

#[test]
fn statement_read_with_nullable() -> sqlite_ll::Result<()> {
    let c = setup_users(":memory:")?;
    let s = "SELECT * FROM users";
    let mut s = c.prepare(s)?;

    assert_eq!(s.step()?, State::Row);
    assert_eq!(s.read::<Option<i64>>(0)?, Some(1));
    assert_eq!(s.read::<Option<String>>(1)?, Some(String::from("Alice")));
    assert_eq!(s.read::<Option<f64>>(2)?, Some(42.69));
    assert_eq!(s.read::<Option<Vec<u8>>>(3)?, Some(vec![0x42, 0x69]));
    assert_eq!(s.read::<Option<String>>(4)?, None);
    assert_eq!(s.step()?, State::Done);
    Ok(())
}

#[test]
fn statement_wildcard() -> sqlite_ll::Result<()> {
    let c = setup_english(":memory:")?;
    let s = "SELECT value FROM english WHERE value LIKE '%type'";
    let mut s = c.prepare(s)?;

    let mut count = 0;

    while let State::Row = s.step()? {
        count += 1;
    }

    assert_eq!(count, 6);
    Ok(())
}

#[test]
fn statement_wildcard_with_binding() -> sqlite_ll::Result<()> {
    let c = setup_english(":memory:")?;
    let s = "SELECT value FROM english WHERE value LIKE ?";
    let mut s = c.prepare(s)?;
    s.bind(1, "%type")?;

    let mut count = 0;
    while let State::Row = s.step()? {
        count += 1;
    }
    assert_eq!(count, 6);
    Ok(())
}

#[test]
fn test_dropped_connection() -> sqlite_ll::Result<()> {
    let c = setup_users(":memory:")?;
    let s = "SELECT id, name, age, photo AS user_photo FROM users";
    let s = c.prepare(s)?;
    drop(c);

    let names = s.column_names()?;
    assert_eq!(names, vec!["id", "name", "age", "user_photo"]);
    assert_eq!("user_photo", s.column_name(3)?);
    Ok(())
}

fn setup_english<T>(path: T) -> sqlite_ll::Result<Connection>
where
    T: AsRef<Path>,
{
    let c = Connection::open(path)?;
    c.execute(
        "
        CREATE TABLE english (value TEXT);
        INSERT INTO english VALUES ('cerotype');
        INSERT INTO english VALUES ('metatype');
        INSERT INTO english VALUES ('ozotype');
        INSERT INTO english VALUES ('phenotype');
        INSERT INTO english VALUES ('plastotype');
        INSERT INTO english VALUES ('undertype');
        INSERT INTO english VALUES ('nonsence');
        ",
    )?;
    Ok(c)
}

fn setup_users<T>(path: T) -> sqlite_ll::Result<Connection>
where
    T: AsRef<Path>,
{
    let c = Connection::open(path)?;
    c.execute(
        "
        CREATE TABLE users (id INTEGER, name TEXT, age REAL, photo BLOB, email TEXT);
        INSERT INTO users VALUES (1, 'Alice', 42.69, X'4269', NULL);
        ",
    )?;
    Ok(c)
}
