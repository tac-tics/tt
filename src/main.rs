use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use interprocess::local_socket::LocalSocketStream;
//use signal_hook::{consts::{SIGTERM, SIGINT}, iterator::Signals};
use simplelog::*;
use log::*;

use termion::raw::IntoRawMode;
use termion::event::Key;
use std::io::{Write, stdout, stdin};
use termion::raw::RawTerminal;
use std::time::Duration;

pub mod message;

use message::{ServerMessage, ClientMessage, Position, Size};

const IPC_DIR: &'static str = "/home/tac-tics/projects/tt/ipc";

fn clear_screen<T: Write>(stdout: &mut RawTerminal<T>) {
    write!(
        stdout,
        "{}{}",
        termion::clear::All,
        termion::cursor::Goto(1, 1),
    ).unwrap();
    stdout.flush().unwrap();
}

fn keyboard_input_thread(sender: mpsc::Sender<ClientEvent>) {
    use termion::input::TermRead;

    for key in stdin().keys() {
        match key {
            Ok(key) => sender.send(ClientEvent::Key(key)).unwrap(),
            Err(e) => error!("Error reading from keyboard input: {e:?}"),
        }
    }
}

fn server_message_received_thread(mut connection: Connection, sender: mpsc::Sender<ClientEvent>) {
    loop {
        if let Some(message) = connection.receive().unwrap() {
            sender.send(ClientEvent::ServerMessageReceived(message)).unwrap();
        }
        std::thread::sleep(std::time::Duration::from_micros(1));
    }
}

fn resize_listener(sender: mpsc::Sender<ClientEvent>) {
    let mut current_size = (1, 1);
    loop {
        let size = termion::terminal_size().unwrap();

        if current_size != size {
            info!("Window size changed is {size:?}");
            sender.send(ClientEvent::Resize(size)).unwrap();
            current_size = size;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

enum ClientEvent {
    Key(Key),
    ServerMessageReceived(message::ServerMessage),
    Resize(Size),
}

#[derive(Clone)]
struct Connection {
    connection: Arc<Mutex<LocalSocketStream>>,
}

impl Connection {
    fn connect() -> Result<Self, ()> {
        let name = format!("{IPC_DIR}/tt.sock");
        let sock_path = std::path::Path::new(&name);
        if sock_path.exists() {
            let socket = LocalSocketStream::connect(sock_path).unwrap();
            socket.set_nonblocking(true).unwrap();
            let connection = Arc::new(Mutex::new(socket));
            Ok(Connection { connection })
        } else {
            Err(())
        }
    }

    fn send(&mut self, message: message::ClientMessage) -> std::io::Result<()> {
        info!("Sending a message soon...");
        let mut conn = self.connection.lock().unwrap();
        info!("Got lock");
        use message::WriteClientMessage;
        debug!("Sending message: {:?}", message);
        conn.write_message(message)
    }

    fn receive(&mut self) -> std::io::Result<Option<message::ServerMessage>> {
        let mut conn = self.connection.lock().unwrap();
        use message::ReadServerMessage;
        match conn.read_message() {
            Ok(message) => {
                debug!("Received message: {:?}", &message);
                return Ok(Some(message));
            },
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_secs(0));
                return Ok(None);
            },
            Err(e) => {
                error!("Error when reading socket: {e:?}");
                return Err(e);
            },
        }
    }
}

fn goto<T: Write>(stdout: &mut T, pos: Position) -> anyhow::Result<()> {
    write!(stdout, "{}", termion::cursor::Goto(pos.0 + 1, pos.1 + 1))?;
    Ok(())
}


fn main() {
    setup_logging();
    info!("Started client");

    /*
    let mut signals = Signals::new(&[SIGINT, SIGTERM]).unwrap();
    std::thread::spawn(move || {
        for sig in signals.forever() {
            error!("Received signal: {:?}", &sig);
        }
    });
    */

    let mut connection: Connection = Connection::connect().unwrap_or_else(|_err| {
        error!("tt-daemon isn't running");
        std::process::exit(1);
    });

    let stdout = stdout();
    let mut stdout = stdout.lock().into_raw_mode().unwrap();

    let (sender, receiver) = mpsc::channel();

    let keyboard_input_thread_receiver = sender.clone();
    let resize_receiver = sender.clone();
    let server_message_receiver = sender;
    let connection2 = connection.clone();
    std::thread::spawn(move || keyboard_input_thread(keyboard_input_thread_receiver));
    std::thread::spawn(move || server_message_received_thread(connection2, server_message_receiver));
    std::thread::spawn(move || resize_listener(resize_receiver));

    clear_screen(&mut stdout);
    connection.send(message::ClientMessage::Connect).unwrap();

    let args: Vec<String> = std::env::args().collect();
    if let Some(filename) = args.get(1).cloned() {
        let filename = std::fs::canonicalize(filename).unwrap().to_str().unwrap().to_string();
        info!("Opening file: {filename:?}");
        connection.send(message::ClientMessage::Open(filename)).unwrap();
    }

    'runloop: loop {
        let event = receiver.recv().unwrap();
        match event {
            ClientEvent::Key(key) => {
                if key == Key::Ctrl('c') {
                    info!("Detected C-c. Exiting.");
                    connection.send(message::ClientMessage::Disconnect).unwrap();
                    break 'runloop;
                } else {
                    let message = message::ClientMessage::SendInput(key.into());
                    connection.send(message).unwrap();
                }
            },
            ClientEvent::ServerMessageReceived(message) => {
                info!("Received message: {message:?}");
                match message {
                    ServerMessage::Update((x, y), (width, height), lines) => {
                        let empty = String::new();

                        for i in 0..height {
                            let line: &str = &lines.get(i as usize).unwrap_or_else(|| &empty);
                            let cur_pos = (x, y + i as u16);
                            goto(&mut stdout, cur_pos).unwrap();
                            for ch in line.chars().take(width as usize) {
                                write!(stdout, "{}", ch).unwrap();
                            }
                            for _ in line.len()..width as usize {
                                write!(stdout, "{}", ' ').unwrap();
                            }
                        }

                        stdout.flush().unwrap();
                    },
                    ServerMessage::Cursor(pos) => {
                        goto(&mut stdout, pos).unwrap();
                        stdout.flush().unwrap();
                    },
                    _ => (),
                }
            },
            ClientEvent::Resize(size) => {
                info!("Issuing resize");
                connection.send(ClientMessage::Resize(size)).unwrap();
            },
        }
    }
    clear_screen(&mut stdout);
    info!("Good-bye!");
}

fn setup_logging() {
    WriteLogger::init(
        LevelFilter::Debug,
        Config::default(),
        std::fs::File::create(format!("{IPC_DIR}/tt.log")).unwrap(),
    ).unwrap();
}
