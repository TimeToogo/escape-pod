use std::net::SocketAddr;

use escapepod_common::{
    proto::EscapeeMessage,
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

    0
}
