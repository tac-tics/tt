use anyhow;
use log::*;
use std::os::unix::io::RawFd;
use nix::sys::socket::{socket, bind, send, recv, accept, listen, connect};
use nix::sys::socket::AddressFamily;
use nix::sys::socket::SockType;
use nix::sys::socket::SockFlag;
use nix::sys::socket::UnixAddr;
use nix::sys::socket::MsgFlags;
use std::fmt::Debug;
use std::io::Write;

use serde::{Serialize, Deserialize};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};


pub struct Listener(RawFd);

impl Listener {
    pub fn listen(sock_path: std::path::PathBuf) -> anyhow::Result<Self> {
        if sock_path.exists() {
            std::fs::remove_file(&sock_path)?;
        }
        let fd = socket(
            AddressFamily::Unix,
            SockType::Stream,
            SockFlag::empty(),
            None,
        )?;
        let addr: UnixAddr = nix::sys::socket::UnixAddr::new(&sock_path)?;
        bind(fd, &addr)?;
        listen(fd, 1)?;
        debug!("Listening on {sock_path:?}");
        Ok(Listener(fd))
    }
}

impl Iterator for Listener {
    type Item = anyhow::Result<Connection>;
    fn next(&mut self) -> std::option::Option<<Self as Iterator>::Item> {
        let fd = accept(self.0).ok()?;
        Some(Ok(Connection(fd)))
    }
}

#[derive(Clone)]
pub struct Connection(RawFd);

impl Connection {
    pub fn connect(sock_path: std::path::PathBuf) -> anyhow::Result<Self> {
        let fd = socket(
            AddressFamily::Unix,
            SockType::Stream,
            SockFlag::empty(), // SockFlag::SOCK_NONBLOCK,
            None,
        )?;
        let addr: UnixAddr = nix::sys::socket::UnixAddr::new(&sock_path)?;
        connect(fd, &addr).unwrap();
        debug!("Connected to {sock_path:?}");
        Ok(Connection(fd))
    }

    pub fn send<T: Serialize + Debug>(&mut self, message: T) -> std::io::Result<()> {
        let fd = self.0;
        let flags = MsgFlags::empty();

        let json_data = serde_json::to_string(&message)?;
        let json_data_len: u64 = json_data.len().try_into().unwrap();

        let mut size_buf = vec![];
        size_buf.write_u64::<LittleEndian>(json_data_len)?;
        let mut total_written = 0;
        while total_written < 8 {
            total_written += send(fd, &size_buf[total_written..], flags)?;
        }

        let mut buf = vec![];
        buf.write(json_data.as_bytes())?;

        let mut total_written = 0;
        while total_written < json_data_len as usize {
            total_written += send(fd, &buf, flags)?;
        }
        debug!("<-- {message:?}");
        Ok(())
    }

    pub fn receive<T: for<'a> Deserialize<'a> + Debug + Clone>(&mut self) -> std::io::Result<Option<T>> {
        let fd = self.0;
        let flags = MsgFlags::empty();

        let mut size_buf = vec![0; 8];
        let mut total_read = 0;

         while total_read < 8 {
            let read = recv(fd, &mut size_buf[total_read..], flags)?;
            if read == 0 {
                return Ok(None);
            }
            total_read += read;
        }
        let mut cursor = std::io::Cursor::new(size_buf);
        let json_data_len: usize = cursor.read_u64::<LittleEndian>().unwrap() as usize;
        let mut message_buf = vec![0u8; json_data_len];

        let mut total_read = 0;
        while total_read < json_data_len {
            let read = recv(fd, &mut message_buf[total_read..], flags)?;
            if read == 0 {
                return Ok(None);
            }
            total_read += read;
        }
        let message: T = serde_json::from_slice::<T>(&mut message_buf)?;
        debug!("--> {message:?}");
        Ok(Some(message))
    }
}
