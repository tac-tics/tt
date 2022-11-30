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
use std::path::{PathBuf, Path};

use tt::connection::{Connection, Listener};
use tt::message::{ClientMessage, ServerMessage, Size, Key, Position};

pub mod render;

const IPC_DIR: &'static str = "/home/tac-tics/projects/tt/ipc";

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone, Copy)]
pub enum BufferMode {
    Normal,
    Insert,
    Command,
}

impl Default for BufferMode {
    fn default() -> Self {
        BufferMode::Normal
    }
}


#[derive(Default, Serialize, Deserialize)]
pub struct Buffer {
    pub path: Option<PathBuf>,
    pub data: String,
    pub pos: Position,
}

#[derive(Default, Serialize, Deserialize)]
pub struct TermTextState {
    pub mode: BufferMode,
    pub buffers: Vec<Buffer>,
    pub command: Option<String>,
    pub size: Size,
}

impl TermTextState {
    pub fn create_buffer(&mut self, path: &Path) -> &mut Buffer {
        if self.buffer_exists_by_path(path) {
            return self.buffer_by_path(path).unwrap();
        }

        let buffer = Buffer {
            path: Some(path.to_path_buf()),
            pos: (0, 0),
            data: "".to_string(),
        };
        self.buffers.push(buffer);
        &mut self.buffers[0]
    }

    pub fn buffer_exists_by_path(&mut self, path: &Path) -> bool {
        let pathbuf = path.to_path_buf();
        for buffer in self.buffers.iter_mut() {
            if buffer.path.as_ref() == Some(&pathbuf) {
                return true;
            }
        }
        false
    }

    pub fn buffer_by_path(&mut self, path: &Path) -> Option<&mut Buffer> {
        let pathbuf = path.to_path_buf();
        for buffer in self.buffers.iter_mut() {
            if buffer.path.as_ref() == Some(&pathbuf) {
                return Some(buffer);
            }
        }
        None
    }

    pub fn close_current_buffer(&mut self) -> bool {
        if self.buffers.len() > 0 {
            self.buffers.remove(0);
            true
        } else {
            false
        }
    }

    pub fn current_buffer(&self) -> Option<&Buffer> {
        if self.buffers.len() > 0 {
            Some(&self.buffers[0])
        } else {
            None
        }
    }

    pub fn current_buffer_mut(&mut self) -> Option<&mut Buffer> {
        if self.buffers.len() > 0 {
            Some(&mut self.buffers[0])
        } else {
            None
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug, Copy)]
struct ConnectedClient {
    connection: Connection,
}

impl ConnectedClient {

}

struct Server {
    state: TermTextState,
    event_sender: mpsc::Sender<ServerEvent>,
    event_receiver: Option<mpsc::Receiver<ServerEvent>>,
}

impl Server {
    fn new() -> Self {
        let (event_sender, event_receiver) = mpsc::channel();
        Server {
            state: TermTextState::default(),
            event_sender,
            event_receiver: Some(event_receiver),
        }
    }

    fn take_event_receiver() -> mpsc::Receiver<ServerEvent> {
        Server::get().event_receiver.take().unwrap()
    }

    fn get<'a>() -> MutexGuard<'a, Self> {
        SERVER.lock().unwrap()
    }

    fn with_state<F, R>(update: F) -> R
        where F: FnOnce(&mut TermTextState) -> R {
        let state = &mut Server::get().state;
        update(state)
    }

    fn connect_client(connection: Connection) {
        let client = ConnectedClient {
            connection,
        };
        let client1 = client.clone();
        std::thread::Builder::new().name("client_message_thread".to_string()).spawn(move || {
            client_message_received_thread(client1).unwrap();
        }).unwrap();
        CLIENTS.lock().unwrap().push(client);

    }

    fn trigger(event: ServerEvent) -> anyhow::Result<()> {
        Server::get().event_sender.send(event)?;
        Ok(())
    }

    fn broadcast(message: ServerMessage) -> anyhow::Result<()> {
        for client in CLIENTS.lock().unwrap().iter_mut() {
            client.connection.send(message.clone())?;
        }
        Ok(())
    }

    fn disconnect_client(client: ConnectedClient) {
        let mut clients = CLIENTS.lock().unwrap();
        let mut i = 0;
        for cur_client in clients.iter() {
            if *cur_client == client {
                clients.swap_remove(i);
                return;
            }
            i += 1;
        }
        panic!("Tried to remove client from server, but client not found.");
    }
}

lazy_static! {
    static ref SERVER: Arc<Mutex<Server>> = Arc::new(Mutex::new(Server::new()));
    static ref CLIENTS: Mutex<Vec<ConnectedClient>> = Mutex::new(vec![]);
}

fn main() {
    setup_logging();
    /* if let Ok(fork::Fork::Child) = fork::daemon(false, false) { */

    let pathname = format!("{IPC_DIR}/tt.pid");
    let path = std::path::Path::new(&pathname);

    let pid = std::process::id();
    trap_signals();
    let mut file = std::fs::File::create(path).unwrap();
    writeln!(file, "{}", pid).unwrap();

    let event_receiver = Server::take_event_receiver();
    let server_event_loop = std::thread::Builder::new().name("server_event_loop".to_string()).spawn(|| {
       server_event_loop_thread(event_receiver).unwrap();
    }).unwrap();

    let client_connection_loop = std::thread::Builder::new().name("client_connection_loop".to_string()).spawn(|| {
        let sock_path = std::path::PathBuf::from( format!("{IPC_DIR}/tt.sock"));
        let listener: Listener = Listener::listen(sock_path).unwrap();
        for incomming_connection in listener {
            info!("Received connection");
            match incomming_connection {
                Ok(connection) => Server::connect_client(connection),
                Err(err) => error!("Error while opening incoming connection: {err}"),
            }
        }
    }).unwrap();

    client_connection_loop.join().unwrap();
    server_event_loop.join().unwrap();

    info!("Exiting");
}

fn setup_logging() {
    WriteLogger::init(
        LevelFilter::Info,
        Config::default(),
        std::fs::File::create(format!("{IPC_DIR}/tt-daemon.log")).unwrap(),
    ).unwrap();
}

fn trap_signals() {
    let mut signals = Signals::new(&[SIGTERM, SIGINT]).unwrap();
    std::thread::spawn(move || {
        for sig in signals.forever() {
            debug!("Received signal: {sig}");
            let pid_file = &format!("{IPC_DIR}/tt.pid");
            let path = std::path::Path::new(pid_file);
            if path.exists() {
                std::fs::remove_file(path).unwrap();
                debug!("Removing PID file: {pid_file}");
            }

            let sock_file = &format!("{IPC_DIR}/tt.sock");
            let path = std::path::Path::new(sock_file);
            if path.exists() {
                std::fs::remove_file(path).unwrap();
                debug!("Removing sock file: {sock_file}");
            }

            info!("Exiting");
            std::process::exit(0)
        }
    });
}

#[derive(Debug)]
enum ServerEvent {
    ClientMessageReceived(ConnectedClient, ClientMessage),
    OpenFile(PathBuf),
    WriteFile(PathBuf),
    CloseFile(),
    IssueCommand(String),
}

fn client_message_received_thread(mut client: ConnectedClient) -> anyhow::Result<()> {
    loop {
        match client.connection.receive() {
            Ok(Some(message)) => {
                Server::trigger(ServerEvent::ClientMessageReceived(client, message)).unwrap();
            },
            Ok(None) => (),
            Err(e) => {
                error!("{e:?}");
                return Err(e.into());
            }
        }

        std::thread::sleep(Duration::from_micros(1));
    }
}

fn send_update() -> anyhow::Result<()> {
    info!("send_update()");

    for message in render::render(&Server::get().state) {
        Server::broadcast(message)?;
    }
    Ok(())
}

fn handle_server_event(event: ServerEvent) -> anyhow::Result<()> {
    match event {
        ServerEvent::ClientMessageReceived(client, message) => {
            info!("Received message: {message:?}");
            match message {
                ClientMessage::Connect(args) => {
                    if args.len() > 0 {
                        let filename = &args[0];
                        Server::trigger(ServerEvent::OpenFile(PathBuf::from(filename)))?;
                    }
                    send_update()?;
                },
                ClientMessage::RequestRefresh => {
                    send_update()?;
                },
                ClientMessage::Disconnect => {
                    Server::with_state(|state| {
                        state.mode = BufferMode::Normal;
                        state.command = None;
                    });

                    Server::disconnect_client(client);
                },
                ClientMessage::SendInput(key) => {
                    handle_input(key)?;
                    send_update()?;
                },
                ClientMessage::Resize(size) => {
                    Server::with_state(|state| {
                        state.size = size;
                    });
                    send_update()?;
                },
            }
        },
        ServerEvent::OpenFile(filepath) => {
            info!("Handling OpenFile({filepath:?})");
            let abs_filepath = filepath.canonicalize()?;
            info!("Abspath: {:?}", abs_filepath);
            if !filepath.exists() {
                std::fs::File::create(&filepath)?;
            }
            let mut file = std::fs::File::open(&filepath)?;
            let mut data = String::new();
            file.read_to_string(&mut data)?;
            Server::with_state(|state| {
                if let Some(buffer) = state.buffer_by_path(&abs_filepath) {
                    buffer.data = data;
                } else {
                    state.create_buffer(&abs_filepath);
                }

            });
            send_update()?;
        },
        ServerEvent::WriteFile(filepath) => {
            info!("Handling WriteFile({filepath:?})");
            let mut file = std::fs::File::options()
                .write(true)
                .truncate(true)
                .create(true)
                .open(filepath)?;

            let state = &mut Server::get().state;
            if let Some(buffer) = &mut state.current_buffer_mut() {
                let data = buffer.data.clone();
                file.write_all(&data.as_bytes())?;
            } else {
                error!("No buffer to write");
            }
        },
        ServerEvent::CloseFile() => {
            info!("Handling CloseFile()");
            Server::with_state(|state| {
                state.close_current_buffer();
            });
            send_update()?;
        },
        ServerEvent::IssueCommand(command) => {
            info!("COMMAND: {command:?}");
            let command_parts: Vec<String> = command.split(' ').map(|s| s.to_owned()).collect();
            if command_parts.len() > 0 {
                if command_parts[0] == "open" {
                    let filename = PathBuf::from(command_parts[1].to_owned());
                    Server::trigger(ServerEvent::OpenFile(filename))?;
                } else if command_parts[0] == "write" {
                    if command_parts.len() > 1 {
                        let filepath = PathBuf::from(command_parts[1].clone());
                        let state = &mut Server::get().state;

                        if let Some(buffer) = state.current_buffer_mut() {
                            buffer.path = Some(filepath);
                        }
                    }
                    let state = &mut Server::get().state;
                    if let Some(buffer) = state.current_buffer_mut() {
                        let path = buffer.path.clone();

                        if let Some(filename) = path {
                            Server::trigger(ServerEvent::WriteFile(filename.clone()))?;
                        }
                    }
                } else if command_parts[0] == "close" {
                    Server::trigger(ServerEvent::CloseFile())?;
                } else if command_parts[0] == "open" && command_parts.len() > 1 {
                    let filename = PathBuf::from(command_parts[1].clone());
                    Server::trigger(ServerEvent::WriteFile(filename))?;
                }
            } else {
                info!("No command matched");
            }
        },
    }
    Ok(())
}

fn server_event_loop_thread(event_receiver: mpsc::Receiver<ServerEvent>) -> anyhow::Result<()> {
    loop {
        let event = event_receiver.recv()?;
        if let Err(e) = handle_server_event(event) {
            error!("{e:?}");
        }
    }
}


fn handle_input(key: Key) -> anyhow::Result<()> {
    let mode = Server::get().state.mode;

    info!("Mode: {:?}    Key: {:?}", mode, key);
    match (mode, key) {
        (BufferMode::Normal, Key::Char('i')) => {
            info!("Changing to insert mode");
            Server::get().state.mode = BufferMode::Insert;
        },
        (BufferMode::Normal, Key::Char(':')) => {
            Server::get().state.mode = BufferMode::Command;
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
                if let Some(buffer) = state.current_buffer_mut() {
                    buffer.data.pop();
                } else {
                    error!("No current buffer");
                }
            });
        },
        (BufferMode::Insert, Key::Char(c)) => {
            Server::with_state(|state| {
                if let Some(buffer) = state.current_buffer_mut() {
                    buffer.data.push(c);
                } else {
                    error!("No current buffer");
                }
            });
        },
        (BufferMode::Command, Key::Char(c)) => {
            if c == '\n' {
                let command = Server::with_state(|state| {
                    state.mode = BufferMode::Normal;
                    state.command.take().unwrap()
                });
                Server::trigger(ServerEvent::IssueCommand(command))?;
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
    send_update()?;
    Ok(())
}
