use std::process;

use escapepod_common::nix::sys::signal::Signal;
use escapepod_tests::util::{escapepod_bin, spawn, wait_for_output};

#[test]
fn it_boots() {
    let code = spawn(&mut process::Command::new(escapepod_bin()))
        .proc
        .wait()
        .unwrap();

    // should fail due to missing args
    assert!(!code.success());
}

#[test]
fn it_exits_with_code_from_child_proc() {
    let code = spawn(
        process::Command::new(escapepod_bin())
            .args(&["--signal", "SIGUSR1"])
            .args(&["--launch-pod-command", "echo"])
            .args(&["--port", "0"])
            .args(&["--", "sh", "-c", "exit 64"]),
    )
    .proc
    .wait()
    .unwrap();

    assert_eq!(code.code(), Some(64));
}

#[test]
fn it_forwards_other_signals_to_child_proc() {
    let mut proc = spawn(
        process::Command::new(escapepod_bin())
            .args(&["--signal", "SIGUSR1"])
            .args(&["--launch-pod-command", "echo"])
            .args(&["--port", "0"])
            .args(&["--", "sleep", "infinity"]),
    );

    wait_for_output(&mut proc, "waiting for signals");

    proc.signal(Signal::SIGINT);
    let code = proc.proc.wait().unwrap();

    assert_eq!(code.code(), Some(130));
}
