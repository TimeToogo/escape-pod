use std::{net::SocketAddr, path::PathBuf};

use bincode::{Decode, Encode};
use libc::{c_int, gid_t, mode_t, pid_t, uid_t};

#[derive(Debug, Clone, PartialEq, Encode, Decode)]
pub enum EscapeeMessage {
    ProcessTrees(Vec<Process>),
    Buffer(Buffer),
    File(File),
    FileData(FileData),
    Done,
}

pub type BufferId = u32;
pub type FileId = u32;

#[derive(Debug, Clone, PartialEq, Encode, Decode)]
pub struct Buffer {
    pub buffer: BufferId,
    pub buf: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Encode, Decode)]
pub struct File {
    pub id: FileId,
    pub uid: uid_t,
    pub gid: gid_t,
    pub mode: mode_t,
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Encode, Decode)]
pub struct FileData {
    pub id: FileId,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Encode, Decode)]
pub struct Process {
    pub pid: pid_t,
    pub mmaps: Vec<MemoryMapping>,
    pub fd_table: Vec<Fd>,
    pub threads: Vec<Thread>,
}

#[derive(Debug, Clone, PartialEq, Encode, Decode)]
pub struct Thread {
    pub tid: pid_t,
    pub uid: uid_t,
    pub gid: gid_t,
    pub reg: Vec<u8>, // libc::user_regs_struct
    pub children: Vec<Process>,
}

#[derive(Debug, Clone, PartialEq, Encode, Decode)]
pub struct MemoryMapping {
    pub tid: pid_t,
    pub address: u64,
    pub len: u64,
    pub perm: c_int,
    pub flags: c_int,
    pub data: MemoryMappingData,
}

#[derive(Debug, Clone, PartialEq, Encode, Decode)]
pub enum MemoryMappingData {
    Buffer(BufferId),
    File(MappedFile),
}

#[derive(Debug, Clone, PartialEq, Encode, Decode)]
pub struct MappedFile {
    pub path: PathBuf,
    pub offset: u64,
}

#[derive(Debug, Clone, PartialEq, Encode, Decode)]
pub struct Fd {
    pub pid: pid_t,
    pub fd: u32,
    pub r#type: FdType,
}

#[derive(Debug, Clone, PartialEq, Encode, Decode)]
pub enum FdType {
    File(FdFile),
    Pipe(FdPipe),
    SocketUnix(FdSocketUnix),
    SocketIp(FdSocketIp),
}

#[derive(Debug, Clone, PartialEq, Encode, Decode)]
pub struct FdFile {
    pub file: PathBuf,
    pub mode: mode_t,
    pub position: u64,
}

#[derive(Debug, Clone, PartialEq, Encode, Decode)]
pub enum FdSocketUnix {
    Bind(PathBuf),
    Connect(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Encode, Decode)]
pub enum FdSocketIp {
    Bind(SocketAddr),
    Connect(SocketAddr),
}

#[derive(Debug, Clone, PartialEq, Encode, Decode)]
pub struct FdPipe {
    pub pipe_id: u32,
    pub half: FdPipeHalf,
}

#[derive(Debug, Clone, PartialEq, Encode, Decode)]
pub enum FdPipeHalf {
    Read,
    Write,
}
