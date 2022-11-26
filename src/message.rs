use std::io::{Write, Read};
use serde::{Serialize, Deserialize};
use serde_json;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use log::error;

pub type Position = (u16, u16);
pub type Size = (u16, u16);


#[derive(Hash, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum ClientMessage {
    Connect,
    RequestRefresh,
    SendInput(Key),
    Resize(Size),
    Disconnect,
}


#[derive(Hash, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum ServerMessage {
    Log(String),
    Update(Position, Size, Vec<String>),
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


#[derive(Hash, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Key {
    Backspace,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    BackTab,
    Delete,
    Insert,
    F(u8),
    Char(char),
    Alt(char),
    Ctrl(char),
    Null,
    Esc,
}


impl From<termion::event::Key> for Key {
    fn from(other: termion::event::Key) -> Key {
        match other {
            termion::event::Key::Backspace => Key::Backspace,
            termion::event::Key::Left => Key::Left,
            termion::event::Key::Right => Key::Right,
            termion::event::Key::Up => Key::Up,
            termion::event::Key::Down => Key::Down,
            termion::event::Key::Home => Key::Home,
            termion::event::Key::End => Key::End,
            termion::event::Key::PageUp => Key::PageUp,
            termion::event::Key::PageDown => Key::PageDown,
            termion::event::Key::BackTab => Key::BackTab,
            termion::event::Key::Delete => Key::Delete,
            termion::event::Key::Insert => Key::Insert,
            termion::event::Key::F(n) => Key::F(n),
            termion::event::Key::Char(c) => Key::Char(c),
            termion::event::Key::Alt(c) => Key::Alt(c),
            termion::event::Key::Ctrl(c) => Key::Ctrl(c),
            termion::event::Key::Null => Key::Null,
            termion::event::Key::Esc => Key::Esc,
            _ => {
                error!("Uknown termion::event::Key");
                panic!("Uknown termion::event::Key")
            }
        }
    }
}
