use serde::{Serialize, Deserialize};
use log::*;

pub type Position = (u16, u16);
pub type Size = (u16, u16);


#[derive(Hash, PartialEq, Eq, Debug, Serialize, Deserialize, Clone)]
pub enum ClientMessage {
    Connect(Vec<String>),
    RequestRefresh,
    SendInput(Key),
    Resize(Size),
    Disconnect,
}


#[derive(Hash, PartialEq, Eq, Debug, Serialize, Deserialize, Clone)]
pub enum ServerMessage {
    Log(String),
    Update(Position, Size, Vec<String>),
    Cursor(Position),
    Shutdown,
}


#[derive(Hash, PartialEq, Eq, Debug, Serialize, Deserialize, Clone, Copy)]
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
