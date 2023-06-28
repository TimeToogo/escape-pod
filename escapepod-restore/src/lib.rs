use std::ffi::c_void;

pub mod restore;

#[derive(Debug)]
pub struct CurrentMmap {
    pub addr: usize,
    pub len: usize,
}

#[derive(Debug)]
pub struct NewMmap {
    pub addr: usize,
    pub len: usize,
    pub prot: i32,
    pub flags: i32,
    pub fd: i32,
    pub offset: usize,
}

pub struct RestoreState {
    // new location to set stack pointer
    pub stack_pointer: *const c_void,
    // ptr to restore function codes
    pub restore_fn: *const c_void,
    // mmaps to unmap
    pub current_mmaps: (usize, *const CurrentMmap),
    // mmaps to create
    pub new_mmaps: (usize, *const NewMmap),
    // restore complete signal fd
    pub fd: i32,
    // current pid
    pub pid: i32,
}
