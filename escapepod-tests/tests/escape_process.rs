use std::process;

use escapepod_common::nix::sys::signal::Signal;
use escapepod_tests::util::{escapepod_bin, spawn, wait_for_output};

#[test]
fn escape_hello_world_binary() {
    let mut origin = spawn(
        process::Command::new(escapepod_bin())
            .args(&["--signal", "SIGUSR1"])
            .args(&["--launch-pod-command"])
            .arg(format!("ESCAPEE_ADDR=localhost:$ESCAPEE_PORT {} --launch-pod-command test --port 0 -- test &", escapepod_bin()))
            .args(&["--port", "0"])
            .args(&["--", "sleep", "infinity"]),
    );

    wait_for_output(&origin, "waiting for signal");

    origin.signal(Signal::SIGUSR1);

    origin.proc.wait().unwrap();
}
