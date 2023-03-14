// Benches copied from https://github.com/stainless-steel/sqlite under the MIT
// license.

use criterion::Criterion;
use sqlite_ll::{Connection, State};

criterion::criterion_group!(benches, read_statement, write_statement);
criterion::criterion_main!(benches);

fn read_statement(bencher: &mut Criterion) {
    let connection = create();
    populate(&connection, 100);

    let mut statement = connection
        .prepare("SELECT * FROM data WHERE a > ? AND b > ?")
        .unwrap();

    bencher.bench_function("read_statement", |b| {
        b.iter(|| {
            statement.reset().unwrap();
            statement.bind(1, 42).unwrap();
            statement.bind(2, 42.0).unwrap();
            while let State::Row = statement.step().unwrap() {
                assert!(statement.read::<i64>(0).unwrap() > 42);
                assert!(statement.read::<f64>(1).unwrap() > 42.0);
            }
        });
    });
}

fn write_statement(bencher: &mut Criterion) {
    let connection = create();
    let mut statement = connection
        .prepare("INSERT INTO data (a, b, c, d) VALUES (?, ?, ?, ?)")
        .unwrap();

    bencher.bench_function("write_statement", |b| {
        b.iter(|| {
            statement.reset().unwrap();
            statement.bind(1, 42).unwrap();
            statement.bind(2, 42.0).unwrap();
            statement.bind(3, 42.0).unwrap();
            statement.bind(4, 42.0).unwrap();
            assert_eq!(statement.step().unwrap(), State::Done);
        });
    });
}

fn create() -> Connection {
    let connection = Connection::open(":memory:").unwrap();
    connection
        .execute("CREATE TABLE data (a INTEGER, b REAL, c REAL, d REAL)")
        .unwrap();
    connection
}

fn populate(connection: &Connection, count: usize) {
    let mut statement = connection
        .prepare("INSERT INTO data (a, b, c, d) VALUES (?, ?, ?, ?)")
        .unwrap();

    for i in 0..count {
        statement.reset().unwrap();
        statement.bind(1, i as i64).unwrap();
        statement.bind(2, i as f64).unwrap();
        statement.bind(3, i as f64).unwrap();
        statement.bind(4, i as f64).unwrap();
        assert_eq!(statement.step().unwrap(), State::Done);
    }
}
