use std::{
    ffi::CString,
    process::{self, Stdio},
    sync::mpsc,
    thread,
};

use escapepod_common::{
    nix::{
        sys::{
            signal::{self, Signal},
            signalfd::SigSet,
            wait::{waitpid, WaitStatus},
        },
        unistd::{execvp, fork, ForkResult, Pid},
    },
    proto::{Buffer, EscapeeMessage, MemoryMappingData},
    tracing::{debug, error, info},
    transport::Server,
};

use crate::args::Args;

mod proc;

pub fn begin(args: Args) -> i32 {
    debug!("starting from fresh");

    let server = Server::listen(([0u8; 4], args.port).into()).expect("failed to bind");

    unsafe {
        match fork().expect("failed to fork") {
            ForkResult::Parent { child } => origin_server(args, server, child),
            ForkResult::Child => {
                origin_entrypoint_exec(args);
                unreachable!()
            }
        }
    }
}

fn origin_server(args: Args, mut server: Server, child: Pid) -> i32 {
    info!("entrypoint process ({child:?}) started");

    enum Event {
        Signal(Signal),
        ChildExited(i32),
    }

    let (tx, rx) = mpsc::channel();

    // ignore all signals by default with the exception of SIGCHILD
    // as POSIX mandates that this will chage waitpid's semantics in a way we do not want.
    unsafe {
        debug!("ignoring signals");
        for signal in SigSet::all().iter().filter(|i| *i != Signal::SIGCHLD) {
            let _ = signal::signal(signal, signal::SigHandler::SigIgn);
        }
    }

    // we have ensure that only our dedicated waiter thread receives the signal
    SigSet::all().thread_block().unwrap();

    thread::spawn({
        let tx = tx.clone();
        let args = args.clone();
        move || {
            debug!("waiting for signals: {:?}", args.signal);
            let sig = SigSet::all().wait().expect("failed to wait for signal");

            if args.signal.contains(&sig) {
                let _ = tx.send(Event::Signal(sig));
            } else if sig != Signal::SIGCHLD {
                debug!("forwarding {:?} to child {:?}", sig, child);
                let _ =
                    signal::kill(child, sig).map_err(|e| error!("failed to forward signal: {e:?}"));
            }
        }
    });

    thread::spawn({
        let tx = tx.clone();
        move || {
            SigSet::all().thread_block().unwrap();
            debug!("waiting on child pid: {:?}", child);
            let status = waitpid(child, None).expect("failed to wait");
            let status = match status {
                WaitStatus::Exited(_, status) => status,
                WaitStatus::Signaled(_, signal, _) => 128 + (signal as i32),
                WaitStatus::Stopped(_, _) => return,
                _ => panic!("unexpected: {status:?}"),
            };
            let _ = tx.send(Event::ChildExited(status));
        }
    });

    match rx.recv().unwrap() {
        Event::Signal(sig) => info!("{sig:?} received"),
        Event::ChildExited(code) => {
            info!("child exited with code {code}");
            return code;
        }
    }

    debug!("running '{}' command", args.launch_pod_command);
    let mut proc = process::Command::new("sh")
        .args(&["-c", &args.launch_pod_command])
        .env("ESCAPEE_PORT", server.port().to_string())
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("failed to spawn launch pod command");

    let status = proc.wait().expect("failed to wait");
    if !status.success() {
        panic!(
            "launch pod command failed with non-zero exit code {}",
            status.code().unwrap()
        );
    }
    debug!("launch pod command executed successfully");

    info!("waiting for connection from destination");
    let mut con = server.accept().unwrap();
    info!("received connection from {}", con.peer_addr());

    let procs = proc::freeze(&args, child).expect("failed to freeze processes");
    con.send(EscapeeMessage::ProcessTrees(procs.clone()))
        .unwrap();
    info!("froze child processes");

    for proc in procs.iter().flat_map(|i| i.self_and_descendents()) {
        for mmap in &proc.mmaps {
            if let MemoryMappingData::Buffer(id) = &mmap.data {
                let data = proc::read_mmap(proc, mmap).expect("failed to read proc mmap");
                con.send(EscapeeMessage::Buffer(Buffer::new(*id, data)))
                    .unwrap();
            }
        }
    }
    info!("sent process memory");

    // todo: files

    for proc in procs.iter().flat_map(|i| i.self_and_descendents()) {
        let _ = proc::kill(proc);
    }

    con.send(EscapeeMessage::Done).unwrap();
    drop(con);

    0
}

unsafe fn origin_entrypoint_exec(args: Args) {
    // todo: new pgrp here?
    // only safe to exec here
    let exec = args
        .exec
        .iter()
        .map(|i| CString::new(i.as_bytes().to_vec()).unwrap())
        .collect::<Vec<_>>();
    execvp(exec[0].as_c_str(), &exec[..]).expect("failed to exec");
    unreachable!()
}
