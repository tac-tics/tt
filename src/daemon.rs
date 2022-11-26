use std::sync::{Arc, Mutex};
use std::sync::mpsc;
use interprocess::local_socket::{LocalSocketListener, LocalSocketStream};
use signal_hook::{consts::SIGTERM, iterator::Signals};
use std::io::{Write, Read};
use simplelog::*;
use anyhow;
use log::{info, warn, error, debug};
use lazy_static::lazy_static;
use serde::{Serialize, Deserialize};
use std::time::Duration;
use std::sync::atomic::{AtomicBool, Ordering};
use std::path::PathBuf;

pub mod message;
use message::{ClientMessage, ServerMessage, Size, Key};

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
pub struct TermTextState {
    pub path: Option<PathBuf>,
    pub command: Option<String>,
    pub data: String,
    pub mode: BufferMode,
}

lazy_static! {
    static ref TERMTEXT: Arc<Mutex<TermTextState>> = Arc::new(Mutex::new(TermTextState::default()));
}

fn main() {
    setup_logging();
    let path = format!("{IPC_DIR}/tt.pid");
    let path = std::path::Path::new(&path);
    if path.exists() {
        warn!("tt.pid file already exists");
        let mut file = std::fs::File::open(path).unwrap();
        let mut buf = String::new();
        file.read_to_string(&mut buf).unwrap();
        warn!("tt.pid contents is {buf}");
    } else if let Ok(fork::Fork::Child) = fork::daemon(false, false) {
        let pid = std::process::id();
        warn!("Forking: PID {pid}");
        trap_signals();
        info!("tt-daemon started");
        let mut file = std::fs::File::create(path).unwrap();
        writeln!(file, "{}", pid).unwrap();

        server_loop();
    }
}

fn setup_logging() {
    WriteLogger::init(
        LevelFilter::Debug,
        Config::default(),
        std::fs::File::create(format!("{IPC_DIR}/tt-daemon.log")).unwrap(),
    ).unwrap();
}

fn trap_signals() {
    let mut signals = Signals::new(&[SIGTERM]).unwrap();
    std::thread::spawn(move || {
        for sig in signals.forever() {
            info!("Received signal: {sig}");
            let pid_file = &format!("{IPC_DIR}/tt.pid");
            let path = std::path::Path::new(pid_file);
            if path.exists() {
                std::fs::remove_file(path).unwrap();
                info!("Removing PID file: {pid_file}");
            }

            let sock_file = &format!("{IPC_DIR}/tt.sock");
            let path = std::path::Path::new(sock_file);
            if path.exists() {
                std::fs::remove_file(path).unwrap();
                info!("Removing sock file: {sock_file}");
            }

            info!("Exiting");
            std::process::exit(0)
        }
    });
}

fn server_loop() {
    let name = format!("{IPC_DIR}/tt.sock");
    let listener = LocalSocketListener::bind(name).unwrap();

    for incomming_connection in listener.incoming() {
        info!("Received connection");
        match incomming_connection {
            Ok(connection) => handle_connection(connection)
                .unwrap_or_else(|err| error!("Error while servicing connection: {err}")),
            Err(err) => error!("Error while opening incoming connection: {err}"),
        }
    }
}

#[derive(Clone)]
struct Connection {
    connection: Arc<Mutex<LocalSocketStream>>,
}

impl Connection {
    fn make(connection: LocalSocketStream) -> Self {
        connection.set_nonblocking(true).unwrap();
        let connection = Arc::new(Mutex::new(connection));
        Connection { connection }
    }

    fn send(&mut self, message: ServerMessage) -> std::io::Result<()> {
        let mut conn = self.connection.lock().unwrap();
        use message::WriteServerMessage;
        debug!("Sending message: {:?}", message);
        conn.write_message(message)
    }

    fn receive(&mut self) -> std::io::Result<Option<message::ClientMessage>> {
        let mut conn = self.connection.lock().unwrap();
        use message::ReadClientMessage;
        match conn.read_message() {
            Ok(message) => {
                debug!("Received message: {:?}", &message);
                return Ok(Some(message));
            },
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                return Ok(None);
            },
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                return Ok(None);
            },
            Err(e) => {
                error!("Error when reading socket: {e:?}");
                return Err(e);
            },
        }
    }
}

#[derive(Debug)]
enum ServerEvent {
    ClientMessageReceived(message::ClientMessage),
    OpenFile(String),
    SaveFile(String),
    IssueCommand(String),
}

fn client_message_received_thread(
    running: Arc<AtomicBool>,
    mut connection: Connection,
    sender: mpsc::Sender<ServerEvent>,
) {
    while running.load(Ordering::SeqCst) {
        if let Some(message) = connection.receive().unwrap() {
            sender.send(ServerEvent::ClientMessageReceived(message)).unwrap();
        }

        std::thread::sleep(Duration::from_micros(1));
    }
}


fn get_termtext_data() -> String {
    let termtext = TERMTEXT.lock().unwrap();
    termtext.data.clone()
}

fn get_termtext_mode() -> BufferMode {
    let termtext = TERMTEXT.lock().unwrap();
    termtext.mode
}

fn send_update(connection: &mut Connection, size: Size) -> anyhow::Result<()> {
    let pos = (0, 0);
    let mut cursor_pos = pos;

    let mut lines = Vec::new();

    let data = get_termtext_data();
    let mut line = String::new();

    for ch in data.chars() {
        if ch == '\n' {
            lines.push(line);
            line = String::new();
            cursor_pos.0 = 0;
            cursor_pos.1 += 1;
        } else if ch == '\t' {
            line.push_str("    ");
            cursor_pos.0 += 4;
        } else {
            line.push(ch);
            cursor_pos.0 += 1;
        }
    }

    lines.push(line);

    connection.send(ServerMessage::Update(pos, size, lines))?;

    let status_pos = (0, size.1 - 1 as u16);
    let status_size = (size.0, 1);
    let mut status_line = "tt: ".to_string();
    {
        let termtext = TERMTEXT.lock().unwrap();
        status_line.push_str(&format!("{:?}", termtext.mode));
        if let Some(command) = &termtext.command {
            status_line.push_str(&format!(" {}", command));
        }
    }
    connection.send(ServerMessage::Update(status_pos, status_size, vec![status_line]))?;

    connection.send(ServerMessage::Cursor(cursor_pos))?;
    Ok(())
}

fn handle_connection(connection: LocalSocketStream) -> anyhow::Result<()> {
    let mut connection = Connection::make(connection);
    let (sender, receiver) = mpsc::channel();
    let running = Arc::new(AtomicBool::new(true));

    let connection1 = connection.clone();
    let sender1 = sender.clone();
    let running1 = running.clone();
    let thread = std::thread::spawn(|| {
        client_message_received_thread(running1, connection1, sender1);
    });

    let mut current_size = (80u16, 50u16);

    'serve_loop: loop {
        info!("About to get next message");
        let message = receiver.recv()?;
        info!("{:?}", &message);
        match &message {
            ServerEvent::ClientMessageReceived(message) => {
                info!("Received message: {message:?}");
                match message {
                    ClientMessage::Connect => {
                        send_update(&mut connection, current_size)?;
                    },
                    ClientMessage::RequestRefresh => {
                        send_update(&mut connection, current_size)?;
                    },
                    ClientMessage::Disconnect => break 'serve_loop,
                    ClientMessage::SendInput(key) => {
                        match (get_termtext_mode(), key) {
                            (BufferMode::Normal, Key::Char('i')) => {
                                let mut termtext = TERMTEXT.lock().unwrap();
                                termtext.mode = BufferMode::Insert;
                            },
                            (BufferMode::Normal, Key::Char(':')) => {
                                let mut termtext = TERMTEXT.lock().unwrap();
                                termtext.mode = BufferMode::Command;
                                termtext.command = Some(String::new());
                            },
                            (_, Key::Esc) => {
                                let mut termtext = TERMTEXT.lock().unwrap();
                                termtext.mode = BufferMode::Normal;
                                termtext.command = None;
                            },
                            (BufferMode::Insert, Key::Backspace) => {
                                let mut termtext = TERMTEXT.lock().unwrap();
                                termtext.data.pop();
                            },
                            (BufferMode::Insert, Key::Char(c)) => {
                                let mut termtext = TERMTEXT.lock().unwrap();
                                termtext.data.push(*c);
                            },
                            (BufferMode::Command, Key::Char(c)) => {
                                if *c == '\n' {
                                    let mut termtext = TERMTEXT.lock().unwrap();
                                    let command = termtext.command.take().unwrap();
                                    termtext.mode = BufferMode::Normal;
                                    sender.send(ServerEvent::IssueCommand(command))?;
                                } else if *c == '\t' {
                                    break;
                                } else {
                                    let mut termtext = TERMTEXT.lock().unwrap();
                                    termtext.command.as_mut().unwrap().push(*c);
                                }
                            },
                            (BufferMode::Command, Key::Backspace) => {
                                let mut termtext = TERMTEXT.lock().unwrap();
                                termtext.command.as_mut().unwrap().pop();
                            },
                            (mode, key) => {
                                info!("Unknown keybind: {mode:?} {key:?}");
                            },
                        }
                        send_update(&mut connection, current_size)?;
                    },
                    ClientMessage::Resize(size) => {
                        current_size = *size;
                        send_update(&mut connection, current_size)?;
                    },
                }
            },
            ServerEvent::OpenFile(filepath) => {
                info!("Handling OpenFile({filepath:?})");
                let mut file = std::fs::File::open(filepath)?;
                let mut data = String::new();
                file.read_to_string(&mut data)?;
                info!("TAKING LOCK");
                {
                    let mut termtext = TERMTEXT.lock().unwrap();
                    info!("File data is now {data}");
                    termtext.data = data;
                }
                send_update(&mut connection, current_size)?;
            },
            ServerEvent::SaveFile(filepath) => {
                info!("Handling SaveFile({filepath:?})");
                let mut file = std::fs::File::options()
                    .write(true)
                    .truncate(true)
                    .open(filepath)?;

                let data: String = get_termtext_data();
                file.write_all(&data.as_bytes())?;
            },
            ServerEvent::IssueCommand(command) => {
                info!("COMMAND: {command:?}");
                let command_parts: Vec<&str> = command.split(' ').collect();
                if command_parts[0] == "open" {
                    let filename = command_parts[1].to_owned();
                    sender.send(ServerEvent::OpenFile(filename))?;
                } else if command_parts[0] == "write" {
                    let filename = command_parts[1].to_owned();
                    sender.send(ServerEvent::SaveFile(filename))?;
                }
            },
        }
    }
    running.store(false, Ordering::SeqCst);
    thread.join().unwrap();
    Ok(())
}
