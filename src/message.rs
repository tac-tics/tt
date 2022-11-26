use std::io::{Write, Read};
use serde::{Serialize, Deserialize};
use serde_json;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};

pub type Position = (u16, u16);
pub type Size = (u16, u16);


#[derive(Hash, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum ClientMessage {
    Connect,
    RequestRefresh,
    SendInput(char),
    Resize(Size),
    Save,
    Disconnect,
}


#[derive(Hash, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum ServerMessage {
    Log(String),
    Update(Position, char),
    Cursor(Position),
    Shutdown,
}


impl<T> ReadClientMessage for T where T: Read {}
impl<T> ReadServerMessage for T where T: Read {}

impl<T> WriteClientMessage for T where T: Write {}
impl<T> WriteServerMessage for T where T: Write {}


pub trait WriteClientMessage: Write {
    fn write_message(&mut self, message: ClientMessage) -> std::io::Result<()> {
        let json_data = serde_json::to_string(&message)?;
        let json_data_len: u64 = json_data.len().try_into().unwrap();
        self.write_u64::<LittleEndian>(json_data_len)?;
        self.write(json_data.as_bytes())?;
        self.flush()?;
        Ok(())
    }
}

pub trait ReadClientMessage: Read {
    fn read_message(&mut self) -> std::io::Result<ClientMessage> {
        let json_data_len: usize = self.read_u64::<LittleEndian>()?.try_into().unwrap();
        let mut message_buf = vec![0u8; json_data_len];
        self.read(&mut message_buf[..json_data_len])?;
        let message: ClientMessage = serde_json::from_slice(&mut message_buf)?;
        Ok(message)
    }
}


pub trait WriteServerMessage: Write {
    fn write_message(&mut self, message: ServerMessage) -> std::io::Result<()> {
        let json_data = serde_json::to_string(&message)?;

        let json_data_len: u64 = json_data.len().try_into().unwrap();
        self.write_u64::<LittleEndian>(json_data_len)?;
        self.write(json_data.as_bytes())?;
        self.flush()?;
        Ok(())
    }
}

pub trait ReadServerMessage: Read {
    fn read_message(&mut self) -> std::io::Result<ServerMessage> {
        let json_data_len: usize = self.read_u64::<LittleEndian>()?.try_into().unwrap();
        // TODO: This sleep seems to prevent a memory allocation error.
        std::thread::sleep(std::time::Duration::from_millis(1));
        let mut message_buf = vec![0u8; json_data_len];
        self.read(&mut message_buf[..json_data_len])?;
        let message: ServerMessage = serde_json::from_slice(&mut message_buf)?;
        Ok(message)
    }
}
