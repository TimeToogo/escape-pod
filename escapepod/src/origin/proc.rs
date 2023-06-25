use std::{
    ffi::c_void,
    io::IoSliceMut,
    mem::{size_of, MaybeUninit},
    slice,
    sync::atomic::{AtomicU32, Ordering},
};

use escapepod_common::{
    anyhow::{bail, Context, Result},
    libc,
    nix::{
        sys::{
            ptrace,
            signal::{self, Signal},
            uio::{process_vm_readv, RemoteIoVec},
        },
        unistd::Pid,
    },
    procfs::{
        self,
        process::{FDTarget, MMapPath},
    },
    proto::{
        Fd, FdFile, FdPipe, FdType, MappedFile, MemoryMapping, MemoryMappingData, Process, Thread,
    },
    tracing::{debug, warn},
};

use crate::args::Args;

static BUFFER_ID: AtomicU32 = AtomicU32::new(0);

pub(crate) fn freeze(_args: &Args, child: Pid) -> Result<Vec<Process>> {
    let mut procs = vec![];
    freeze_proc_recursive(child, &mut procs)?;

    Ok(vec![parse_proc_recursive(child)?])
}

// todo: get active processes from preload over socket
fn freeze_proc_recursive(pid: Pid, procs: &mut Vec<procfs::process::Process>) -> Result<()> {
    signal::kill(pid, Signal::SIGSTOP)?;
    let proc = procfs::process::Process::new(pid.as_raw())?;

    for thread in proc.tasks()? {
        let thread = thread?;
        for child in thread.children()? {
            freeze_proc_recursive(Pid::from_raw(child as _), procs)?;
        }
    }

    procs.push(proc);
    Ok(())
}

fn parse_proc_recursive(pid: Pid) -> Result<Process> {
    let proc = procfs::process::Process::new(pid.as_raw())?;

    let fd_table = proc
        .fd()?
        .into_iter()
        .map(|f| {
            f.map(|f| Fd {
                fd: f.fd,
                mode: f.mode as _,
                r#type: match f.target {
                    FDTarget::Path(f) => FdType::File(FdFile {
                        file: f,
                        position: 0, // todo
                    }),
                    FDTarget::Socket(_) => todo!(),
                    FDTarget::Net(_) => todo!(),
                    FDTarget::Pipe(id) => FdType::Pipe(FdPipe { pipe_id: id }),
                    FDTarget::AnonInode(_) => todo!(),
                    FDTarget::MemFD(_) => todo!(),
                    FDTarget::Other(_, _) => todo!(),
                },
            })
            .context("fd")
        })
        .collect::<Result<Vec<_>>>()?;

    let mmaps = proc
        .maps()?
        .into_iter()
        .map(|m| MemoryMapping {
            address: m.address.0,
            len: m.address.1 - m.address.0,
            perm: m.perms.bits() as _,
            data: match m.pathname {
                MMapPath::Vvar => MemoryMappingData::KernelVvar,
                _ => MemoryMappingData::Buffer(BUFFER_ID.fetch_add(1, Ordering::Relaxed)),
            },
        })
        .collect();

    let proc = Process {
        pid: proc.pid(),
        mmaps,
        fd_table,
        threads: proc
            .tasks()?
            .into_iter()
            .map(|t| {
                t.context("task").and_then(|t| {
                    let status = t.status()?;
                    Ok(Thread {
                        tid: t.tid,
                        uid: status.euid,
                        gid: status.egid,
                        reg: get_thread_regset(&t)?,
                        children: t
                            .children()?
                            .into_iter()
                            .map(|i| parse_proc_recursive(Pid::from_raw(i as _)))
                            .collect::<Result<_>>()?,
                    })
                })
            })
            .collect::<Result<Vec<_>>>()?,
    };

    Ok(proc)
}

fn get_thread_regset(t: &procfs::process::Task) -> Result<Vec<u8>> {
    // todo: avoid using ptrace
    let pid = Pid::from_raw(t.pid);

    ptrace::attach(pid)?;

    let reg = unsafe {
        let mut regset: libc::user_regs_struct = MaybeUninit::zeroed().assume_init();
        let mut io: libc::iovec = MaybeUninit::zeroed().assume_init();
        io.iov_base = &mut regset as *mut _ as *mut _;
        io.iov_len = size_of::<libc::user_regs_struct>();
        let res = libc::ptrace(
            libc::PTRACE_GETREGSET,
            pid,
            libc::NT_PRSTATUS as *mut c_void,
            &mut io as *mut _,
        );
        if res != 0 {
            bail!("PTRACE_GETREGSET failed");
        }

        let mut reg = vec![];
        reg.extend_from_slice(slice::from_raw_parts(io.iov_base as *const u8, io.iov_len));
        reg
    };

    ptrace::detach(pid, None)?;

    Ok(reg)
}

pub(crate) fn read_mmap(proc: &Process, mmap: &MemoryMapping) -> Result<Vec<u8>> {
    let mut buf = vec![0u8; mmap.len as _];
    let mut remote_iov = vec![RemoteIoVec {
        base: mmap.address as _,
        len: mmap.len as _,
    }];

    // since we only have one iovec element this cannot result in a partial read
    process_vm_readv(
        Pid::from_raw(proc.pid),
        &mut [IoSliceMut::new(buf.as_mut_slice())],
        remote_iov.as_mut_slice(),
    )?;

    Ok(buf)
}

pub(crate) fn kill(proc: &Process) -> Result<()> {
    match signal::kill(Pid::from_raw(proc.pid), Signal::SIGKILL) {
        Ok(_) => debug!("killed {}", proc.pid),
        Err(e) => warn!("could not kill {}: {:?}", proc.pid, e),
    }

    Ok(())
}
