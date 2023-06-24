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

impl Buffer {
    pub fn new(buffer: BufferId, buf: Vec<u8>) -> Self {
        Self { buffer, buf }
    }
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
impl Process {
    pub fn self_and_descendents(&self) -> Vec<&Process> {
        let mut procs = vec![self];

        for t in &self.threads {
            for p in t.children.iter() {
                procs.extend_from_slice(p.self_and_descendents().as_slice());
            }
        }

        return procs;
    }
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
    pub address: u64,
    pub len: u64,
    pub perm: c_int,
    #[bincode(with_serde)]
    pub r#type: procfs::process::MMapPath,
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
    pub fd: i32,
    pub mode: u16,
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
    pub pipe_id: u64,
}
