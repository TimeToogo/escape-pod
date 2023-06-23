use std::{net::SocketAddr, path::PathBuf};

use libc::{gid_t, mode_t, pid_t, uid_t};
use nix::sys::{
    mman::{MapFlags, ProtFlags},
    stat::Mode,
};

pub enum EscapeeMessage {
    ProcessTrees(Vec<Process>),
    File(File),
    Buffer(Buffer),
    Done,
}

pub type BufferId = u32;
pub type FileId = u32;

pub struct Buffer {
    pub buffer: BufferId,
    pub buf: Vec<u8>,
}

pub struct File {
    pub id: FileId,
    pub uid: uid_t,
    pub gid: gid_t,
    pub mode: Mode,
    pub path: PathBuf,
}

pub struct FileData {
    pub id: FileId,
    pub data: Vec<u8>,
}

pub struct Process {
    pub pid: pid_t,
    pub threads: Vec<Thread>,
    pub fd_table: Vec<Fd>,
}

pub struct Thread {
    pub tid: pid_t,
    pub uid: uid_t,
    pub gid: gid_t,
    pub mmaps: Vec<MemoryMapping>,
    pub reg: libc::user_regs_struct,
    pub children: Vec<Process>,
}

pub struct MemoryMapping {
    pub tid: pid_t,
    pub address: u64,
    pub len: u64,
    pub perm: ProtFlags,
    pub flags: MapFlags,
    pub data: MemoryMappingData,
}

pub enum MemoryMappingData {
    Buffer(BufferId),
    File(MappedFile),
}

pub struct MappedFile {
    pub path: PathBuf,
    pub offset: u64,
}

pub struct Fd {
    pub pid: pid_t,
    pub fd: u32,
    pub r#type: FdType,
}

pub enum FdType {
    File(FdFile),
    Pipe(FdPipe),
    SocketUnix(FdSocketUnix),
    SocketIp(FdSocketIp),
}

pub struct FdFile {
    pub file: PathBuf,
    pub mode: mode_t,
    pub position: u64,
}

pub enum FdSocketUnix {
    Bind(PathBuf),
    Connect(PathBuf),
}

pub enum FdSocketIp {
    Bind(SocketAddr),
    Connect(SocketAddr),
}

pub struct FdPipe {
    pub target_pid: pid_t,
    pub target_fd: u32,
}
