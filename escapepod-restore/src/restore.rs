use std::{
    arch::asm,
    env,
    mem::{self, size_of},
    ptr,
};

use crate::{CurrentMmap, NewMmap, RestoreState};
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

macro_rules! assert {
    ($expr:expr) => {
        if !($expr) {
            let pid = syscalls::raw::syscall0(Sysno::getpid);
            syscalls::raw::syscall2(Sysno::kill, pid, Signal::SIGABRT as _);
        }
    };
}

// this is our restore function
// its goals is to restore the memory mappings to the state before the process froze
#[inline(never)]
pub unsafe extern "C" fn restore(state: *const RestoreState) {
    let state = mem::transmute::<_, &'static RestoreState>(state);

    // stage 1: unmap existing memory mappings (except the restore state and code)
    let (len, mmaps) = state.current_mmaps;
    for i in 0..len {
        let mmap = mmaps.add(i).read_unaligned();

        let res = syscalls::raw::syscall2(Sysno::munmap, mmap.addr, mmap.len);
        assert!(res == 0);
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
        assert!(res == 0);
    }

    // stage 3: signal main process that the mmaps have been restored
    let res = syscalls::raw::syscall3(Sysno::write, state.fd as _, &state as *const _ as usize, 1);
    assert!(res == 0);

    // stage 4: stop the current process
    syscalls::raw::syscall2(Sysno::kill, state.pid as _, Signal::SIGSTOP as _);
}
