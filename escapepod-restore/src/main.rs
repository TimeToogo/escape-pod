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
use syscalls::Sysno;

const RESTORE_SPACE: usize = 100 * 1024;

#[derive(Debug)]
pub struct CurrentMmap {
    addr: usize,
    len: usize,
}

#[derive(Debug)]
pub struct NewMmap {
    addr: usize,
    len: usize,
    prot: i32,
    flags: i32,
    fd: i32,
    offset: usize,
}

pub struct RestoreState {
    current_mmaps: (usize, *const CurrentMmap),
    // mmaps to create
    new_mmaps: (usize, *const NewMmap),
    // restore complete signal fd
    fd: i32,
    // current pid
    pid: i32,
}

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
        let state = RestoreState {
            pid: proc.pid,
            fd: ready_fd,
            current_mmaps: (
                current_mmaps.len(),
                restore_addr.add(size_of::<RestoreState>()) as _,
            ),
            new_mmaps: (
                new_mmaps.len(),
                restore_addr
                    .add(size_of::<RestoreState>())
                    .add(size_of::<CurrentMmap>() * current_mmaps.len()) as _,
            ),
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

        trace!("copied restore state");

        // copy restore code
        let (start, end) = restore_fn_code();
        trace!(
            "restore fn addr {start:?}-{end:?} ({})",
            end.offset_from(start)
        );
        memcpy(
            (state.new_mmaps.1 as usize + size_of::<NewMmap>() * state.new_mmaps.0) as _,
            start as _,
            end.offset_from(start) as _,
        );
        trace!("restore fn copied");
    }
    // copy restore routine to mmap, jump there, fix stack?, unmap all, remap new, signal done

    // trick compiler into not eliding the restore fn
    unsafe {
        restore(0 as _);
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

// this is our restore function
// its goals is to restore the memory mappings to the state before the process froze
#[no_mangle]
#[inline(never)]
pub unsafe extern "C" fn restore(state: *const RestoreState) {
    let state = mem::transmute::<_, &'static RestoreState>(state);

    // stage 1: unmap existing memory mappings (except the restore state and code)
    let (len, mmaps) = state.current_mmaps;
    for i in 0..len {
        let mmap = mmaps.add(i).read_unaligned();

        let res = syscalls::raw::syscall2(Sysno::munmap, mmap.addr, mmap.len);
        // assert!(res == 0);
    }

    // stage 2: recreate process maps
    let (len, mmaps) = state.new_mmaps;
    for i in 0..len {
        let mmap = mmaps.add(i).read_unaligned();

        let res = syscalls::raw::syscall6(
            Sysno::mmap,
            mmap.addr,
            mmap.len,
            mmap.prot as _,
            mmap.flags as _,
            mmap.fd as _,
            mmap.offset,
        );
        // assert!(res == 0);
    }

    // stage 3: signal main process that the mmaps have been restored
    let res = syscalls::raw::syscall3(Sysno::write, state.fd as _, &state as *const _ as usize, 1);
    // assert!(res == 1);

    // stage 4: stop the current process
    syscalls::raw::syscall2(Sysno::kill, state.pid as _, Signal::SIGSTOP as _);

    // end function marker
    asm!("ret", "ret", "ret", "ret", "ret", "ret", "ret", "ret", "ret", "ret")
}

#[cfg(target_arch = "aarch64")]
unsafe fn restore_fn_code() -> (*const u8, *const u8) {
    // todo: i'm sorry
    // let addr = &restore as *const u8;
    #[allow(overflowing_literals)]
    let ret = 0xC0035FD6;

    let start = &restore as *const _ as *const u8;
    let mut addr = start as *const i32;
    let mut rets = 0;
    let mut last = 0;
    let mut i = 0;
    let end = loop {
        trace!("{:#02x}", *addr);
        if *addr == last {
            rets += 1;
        } else {
            rets = 0;
            last = *addr;
        }

        if rets == 5 || i == 540 / 4 {
            trace!("last: {}", last);
            break addr.sub(rets) as *const _;
        }


        addr = addr.add(1);
        i += 1;
    };

    (start, end)
}
