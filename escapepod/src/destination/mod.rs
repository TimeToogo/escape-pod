use std::{
    ffi::{CStr, CString},
    net::SocketAddr,
    os::fd::RawFd,
};

use escapepod_common::{
    anyhow::Result,
    nix::{
        self,
        fcntl::OFlag,
        unistd::{close, execvpe, fork, pipe2, ForkResult, Pid},
    },
    proto::{EscapeeMessage, Process},
    serde_json,
    tracing::{debug, info},
    transport::Client,
};

use crate::args::Args;

pub fn receive(args: Args, addr: SocketAddr) -> i32 {
    info!("connecting to origin {addr:?}");
    let mut client = Client::connect(addr).expect("failed to connect to origin server");
    debug!("connected succesfully");

    info!("waiting for process tree");
    let msg = client
        .recv::<EscapeeMessage>()
        .expect("failed to read first message");

    let procs = match msg {
        EscapeeMessage::ProcessTrees(i) => i,
        msg => panic!("unexpected server message: {msg:?}"),
    };

    let procs = procs
        .into_iter()
        .map(spawn)
        .collect::<Result<Vec<_>>>()
        .unwrap();

    for (pid, ready_fd) in procs {
        let mut buf = [0];
        let read = nix::unistd::read(ready_fd, &mut buf[..]).unwrap();

        if read == 1 {
            info!("{} is ready", pid);
        }
    }

    // todo: restore memory

    0
}

fn spawn(proc: Process) -> Result<(Pid, i32)> {
    let restore_path = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .join("escapepod-restore")
        .to_string_lossy()
        .to_string();

    let (ready_fd_read, ready_fd_write) = pipe2(OFlag::empty()).unwrap();

    // todo restore pid

    unsafe {
        match fork().expect("failed to fork") {
            ForkResult::Parent { child } => {
                info!("forked to pid: {:?}", child);
                return Ok((child, ready_fd_read));
            }
            ForkResult::Child => {
                execvpe(
                    CString::new(restore_path).unwrap().as_c_str(),
                    &[] as &[&CStr],
                    &[
                        CString::new(format!(
                            "EP_PROCESS={}",
                            serde_json::to_string(&proc).unwrap()
                        ))
                        .unwrap()
                        .as_c_str(),
                        CString::new(format!("EP_READY_FD={}", ready_fd_write))
                            .unwrap()
                            .as_c_str(),
                        // todo:
                        CString::new("RUST_LOG=trace").unwrap().as_c_str(),
                    ],
                )
                .unwrap();
                unreachable!()
            }
        }
    }
}
