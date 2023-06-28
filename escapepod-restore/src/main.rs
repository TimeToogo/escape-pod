use std::{
    arch::asm,
    env,
    mem::{self, size_of},
    ptr,
};

use escapepod_common::{
    libc::memcpy,
    nix::{
        self,
        fcntl::OFlag,
        sys::{
            mman::{mmap, MapFlags, ProtFlags},
            signal::Signal,
            stat::Mode,
        },
        unistd::{getpid, sysconf, SysconfVar},
    },
    procfs::{self},
    proto::{FdType, MemoryMappingData, Process},
    serde_json,
    tracing::trace,
};
use escapepod_restore::{restore, CurrentMmap, NewMmap, RestoreState};
use syscalls::Sysno;

const RESTORE_SPACE: usize = 100 * 1024;

// const RESTORE_FN_CODE: &[u8] = include_bytes!("../target/realease/escapepod-restore-inner.s");

fn main() {
    escapepod_common::tracing::init();

    let mut proc =
        serde_json::from_str::<Process>(env::var("EP_PROCESS").unwrap().as_str()).unwrap();

    let mut ready_fd = env::var("EP_READY_FD").unwrap().parse::<i32>().unwrap();

    // todo: restore pid, euid, egid, fds, threads, forks ...
    // close existings file descriptors except ready_fd
    let max_fd = nix::fcntl::open("/dev/null", OFlag::O_RDONLY, Mode::empty()).unwrap();
    for i in 0..=max_fd {
        if i != ready_fd {
            // let _ = close(i);
        }
    }

    // restore file descriptors
    for fd in proc.fd_table.iter() {
        let nfd = match &fd.r#type {
            FdType::File(p) => {
                nix::fcntl::open(
                    p.file.as_path(),
                    // todo: oflag
                    OFlag::O_RDONLY,
                    Mode::from_bits(fd.mode).unwrap(),
                )
                .unwrap()
            }
            _ => continue
            // FdType::Pipe(_) => todo!(),
            // FdType::SocketUnix(_) => todo!(),
            // FdType::SocketIp(_) => todo!(),
        };

        // ensure ready fd does not conflict
        if ready_fd == fd.fd {
            ready_fd = nix::unistd::dup(ready_fd).unwrap();
        }

        nix::unistd::dup2(nfd, fd.fd).unwrap();
    }

    // get memory restore state
    let current_mmaps = procfs::process::Process::new(getpid().as_raw())
        .unwrap()
        .maps()
        .unwrap()
        .into_iter()
        .map(|i| CurrentMmap {
            addr: i.address.0 as _,
            len: (i.address.1 - i.address.0) as _,
        })
        .collect::<Vec<_>>();

    trace!("current_mmaps: {:?}", current_mmaps);

    let new_mmaps = proc
        .mmaps
        .iter()
        .map(|m| {
            let mut mmap = NewMmap {
                addr: m.address as _,
                len: m.len as _,
                prot: m.perm,
                // TODO
                flags: 0,
                fd: 0,
                offset: 0,
            };
            if let MemoryMappingData::File(f) = &m.data {
                mmap.fd = f.fd;
                mmap.offset = f.offset as _;
            }
            mmap
        })
        .collect::<Vec<_>>();

    trace!("new_mmaps: {:?}", new_mmaps);

    // allocate space for restore routine
    let (start, len) = find_safe_address_space(&mut proc);
    trace!("restore address space: ({}, {})", start, start + len);

    let restore_addr = unsafe {
        mmap(
            Some(start.try_into().unwrap()),
            len.try_into().unwrap(),
            ProtFlags::PROT_READ | ProtFlags::PROT_WRITE | ProtFlags::PROT_EXEC,
            MapFlags::MAP_PRIVATE | MapFlags::MAP_FIXED | MapFlags::MAP_ANONYMOUS,
            0,
            0,
        )
        .unwrap()
    };

    trace!("restore_addr: {:?}", restore_addr);

    // first copy the required state to the new mmapped region
    unsafe {
        let ptr = restore_addr;
        let current_mmaps_addr = restore_addr.add(size_of::<RestoreState>());
        let new_mmaps_addr = current_mmaps_addr.add(size_of::<CurrentMmap>() * current_mmaps.len());
        let restore_fn_addr = new_mmaps_addr.add(size_of::<NewMmap>() * new_mmaps.len());
        let restore_fn_addr = restore_fn_addr.add(restore_fn_addr.align_offset(8));
        let stack_pointer_addr = restore_fn_addr.add(size_of::<NewMmap>() * new_mmaps.len());
        let stack_pointer_addr = stack_pointer_addr.add(stack_pointer_addr.align_offset(8));

        let state = RestoreState {
            pid: proc.pid,
            fd: ready_fd,
            current_mmaps: (current_mmaps.len(), current_mmaps_addr as _),
            new_mmaps: (new_mmaps.len(), new_mmaps_addr as _),
            restore_fn: restore_fn_addr,
            stack_pointer: stack_pointer_addr,
        };

        memcpy(
            restore_addr,
            &state as *const _ as _,
            size_of::<RestoreState>(),
        );
        memcpy(
            state.current_mmaps.1 as _,
            &current_mmaps as *const _ as _,
            size_of::<CurrentMmap>() * state.current_mmaps.0,
        );
        memcpy(
            state.new_mmaps.1 as _,
            &new_mmaps as *const _ as _,
            size_of::<NewMmap>() * state.new_mmaps.0,
        );

        let (start, end) = restore_fn_code();
        trace!(
            "restore fn addr {start:?}-{end:?} ({})",
            end.offset_from(start)
        );
        memcpy(
            state.restore_fn as _,
            start as _,
            end.offset_from(start) as _,
        );
        trace!("restore fn copied");

        #[cfg(target_arch = "aarch64")]
        {
            // set the stack pointer to the new address and call the restore function
            unsafe {
                asm!(
                    "mov x0, {r}",
                    "msr el_sp0, {s}",
                    "br {f}",
                    r = in(reg) restore_addr,
                    s = in(reg) state.stack_pointer,
                    f = in(reg) state.restore_fn
                );
            }
        }
    }

    // copy restore routine to mmap, jump there, fix stack?, unmap all, remap new, signal done

    unsafe {
        restore::restore(ptr::null());
    }
}

fn find_safe_address_space(proc: &mut Process) -> (usize, usize) {
    let page_size = sysconf(SysconfVar::PAGE_SIZE).unwrap().unwrap() as usize;
    trace!("page_size: {page_size}");
    let restore_space = RESTORE_SPACE - (RESTORE_SPACE % page_size) + page_size;

    proc.mmaps.sort_by_key(|m| m.address);
    let prev = proc.mmaps.first().unwrap();
    for mmap in proc.mmaps.iter().skip(1) {
        if (mmap.address - prev.address_end()) as usize > restore_space + page_size * 2 {
            assert!(prev.address_end() as usize % page_size == 0);
            let start = prev.address_end() as usize + page_size;

            return (start as _, restore_space);
        }
    }

    panic!("could not find suitable address space")
}

#[cfg(target_arch = "aarch64")]
unsafe fn restore_fn_code() -> (*const u8, *const u8) {
    todo!()
}
