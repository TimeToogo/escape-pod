use std::{
    io::{BufReader, Write},
    net::{SocketAddr, TcpListener, TcpStream},
};

use anyhow::{Context, Result};

pub struct Server {
    listener: TcpListener,
}

impl Server {
    pub fn listen(addr: SocketAddr) -> Result<Self> {
        let listener = TcpListener::bind(addr).context("failed to bind")?;

        Ok(Self { listener })
    }

    pub fn accept(&mut self) -> Result<ServerConnection> {
        let (socket, addr) = self.listener.accept().context("failed to accept")?;

        Ok(ServerConnection::new(socket, addr))
    }

    pub fn port(&self) -> u16 {
        self.listener.local_addr().unwrap().port()
    }
}

pub struct ServerConnection {
    socket: TcpStream,
    peer_addr: SocketAddr,
    bincode_conf: bincode::config::Configuration,
}

impl ServerConnection {
    pub fn new(socket: TcpStream, peer_addr: SocketAddr) -> Self {
        let bincode_conf = bincode::config::standard();
        Self {
            socket,
            peer_addr,
            bincode_conf,
        }
    }

    pub fn peer_addr(&self) -> SocketAddr {
        self.peer_addr
    }

    pub fn send(&mut self, msg: impl bincode::Encode) -> Result<()> {
        let buf = bincode::encode_to_vec(msg, self.bincode_conf)?;
        Ok(self.socket.write_all(buf.as_slice())?)
    }
}

pub struct Client {
    socket: BufReader<TcpStream>,
    bincode_conf: bincode::config::Configuration,
}

impl Client {
    pub fn new(socket: TcpStream) -> Self {
        let bincode_conf = bincode::config::standard();
        Self {
            socket: BufReader::new(socket),
            bincode_conf,
        }
    }

    pub fn connect(addr: SocketAddr) -> Result<Self> {
        let socket = TcpStream::connect(addr).context("failed to connect")?;
        Ok(Self::new(socket))
    }

    pub fn recv<R: bincode::Decode>(&mut self) -> Result<R> {
        Ok(bincode::decode_from_reader(
            &mut self.socket,
            self.bincode_conf,
        )?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_and_client() {
        let mut server = Server::listen(([0u8; 4], 12345).into()).unwrap();
        let mut client = Client::connect(([127, 0, 0, 1], 12345).into()).unwrap();
        let mut con = server.accept().unwrap();

        con.send("test").unwrap();
        assert_eq!(client.recv::<String>().unwrap(), "test".to_string());
    }
}
