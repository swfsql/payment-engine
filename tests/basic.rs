use std::path::PathBuf;

fn run(path: &str, expected: &str) {
    let path = PathBuf::from(path);
    let inputs = payment_engine::read_input_file(&path).unwrap();
    let clients = payment_engine::run(inputs.into_iter());
    let mut clients: Vec<_> = clients.values().collect();
    clients.sort_by(|a, b| a.id.cmp(&b.id));
    let mut output = Vec::new();
    payment_engine::write_output(clients.into_iter().cloned(), &mut output).unwrap();
    let output = String::from_utf8(output).unwrap();
    assert_eq!(
        expected.lines().map(|l| l.trim()).collect::<String>(),
        output.lines().map(|l| l.trim()).collect::<String>()
    );
}

#[test]
fn basic_empty() {
    run("tests/basic_empty.csv", "");
}

#[test]
fn basic_deposits() {
    run(
        "tests/basic_deposits.csv",
        "client,available,held,total,locked
    1,1,0,1,false
    2,1,0,1,false
    3,2,0,2,false",
    );
}

#[test]
fn basic_withdrawal() {
    run(
        "tests/basic_withdrawal.csv",
        "client,available,held,total,locked
    2,0,0,0,false",
    );
}

#[test]
fn basic_dispute() {
    run(
        "tests/basic_dispute.csv",
        "client,available,held,total,locked
    2,0,1,1,false",
    );
}

#[test]
fn basic_resolving() {
    run(
        "tests/basic_resolve.csv",
        "client,available,held,total,locked
    2,1,0,1,false",
    );
}

#[test]
fn basic_chargeback() {
    run(
        "tests/basic_chargeback.csv",
        "client,available,held,total,locked
    2,0,0,0,true",
    );
}
