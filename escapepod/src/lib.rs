pub mod args;
pub mod destination;
pub mod origin;

use std::{
    env,
    net::{SocketAddr, ToSocketAddrs},
    process,
};

use crate::args::Args;
use clap::Parser;

pub fn main() {
    escapepod_common::tracing::init();
    let args = Args::parse();
    assert!(args.exec.len() > 0);

    let code = if let Ok(addr) = env::var("ESCAPEE_ADDR") {
        let addr: SocketAddr = addr
            .to_socket_addrs()
            .expect(&format!("failed to parse ESCAPE_ADDR={addr} var"))
            .into_iter()
            .next()
            .unwrap();
        crate::destination::receive(args, addr)
    } else {
        crate::origin::begin(args)
    };

    process::exit(code);
}
