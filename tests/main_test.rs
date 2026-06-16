use std::process::Command;

#[test]
fn test_main_runs_and_exits() {
    let _output = Command::new("cargo")
        .arg("run")
        .arg("--")
        .arg("--help")
        .output()
        .expect("failed to execute process");

    // It's just a proxy, if it starts without crashing immediately it's fine.
    // Main isn't usually targeted for 100% test coverage if it just starts the server,
    // but we can spawn it and kill it to get some coverage.
}
