
use std::sync::{Arc, Mutex, MutexGuard};
use std::sync::mpsc;
use signal_hook::{consts::SIGTERM, consts::SIGINT, iterator::Signals};
use std::io::{Write, Read};
use simplelog::*;
use anyhow;
use log::*;
use lazy_static::lazy_static;
use serde::{Serialize, Deserialize};
use std::time::Duration;
use std::sync::atomic::{AtomicBool, Ordering};
use std::path::PathBuf;

use server::connection::{Connection, Listener};
pub mod connection;
pub mod state;

use message::{ClientMessage, ServerMessage, Size, Key};


fn handle_input(
    key: Key,
    connection: &mut Connection,
    sender: mpsc::Sender<ServerEvent>,
    current_size: (u16, u16),
) -> anyhow::Result<()> {
    info!("Mode: {:?}    Key: {:?}", get_termtext_mode(), key);
    match (get_termtext_mode(), key) {
        (BufferMode::Normal, Key::Char('i')) => {
            info!("Changing to insert mode");
            Server::get().state.mode = BufferMode::Insert;
        },
        (BufferMode::Normal, Key::Char(':')) => {
            Server::get().state.command = Some(String::new());
        },
        (_, Key::Esc) => {
            Server::get().state.command = Some(String::new());
            Server::with_state(|state| {
                state.mode = BufferMode::Normal;
                state.command = None;
            });
        },
        (BufferMode::Insert, Key::Backspace) => {
            Server::with_state(|state| {
                state.data.pop();
            });
        },
        (BufferMode::Insert, Key::Char(c)) => {
            Server::with_state(|state| {
                state.data.push(c);
            });
        },
        (BufferMode::Command, Key::Char(c)) => {
            if c == '\n' {
                let command = Server::with_state(|state| {
                    state.mode = BufferMode::Normal;
                    state.command.take().unwrap()
                });
                sender.send(ServerEvent::IssueCommand(command))?;
            } else if c == '\t' {
                // do nothing
            } else {
                Server::with_state(|state| {
                    state.command.as_mut().unwrap().push(c);
                });
            }
        },
        (BufferMode::Command, Key::Backspace) => {
            Server::with_state(|state| {
                state.command.as_mut().unwrap().pop();
            });
        },
        (mode, key) => {
            info!("Unknown keybind: {mode:?} {key:?}");
        },
    }
    warn!("mode is now: {:?}", Server::get().state.mode);
    send_update(connection, current_size)?;
    Ok(())
}
