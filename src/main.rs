use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use interprocess::local_socket::{LocalSocketListener, LocalSocketStream};
use signal_hook::{consts::{SIGTERM, SIGINT}, iterator::Signals};
use simplelog::*;
use log::{info, warn, error, debug};

use termion::raw::IntoRawMode;
use termion::event::Key;
use std::io::{Read, Write, stdout, stdin};
use std::thread;
use termion::raw::RawTerminal;
use std::time::Duration;

pub mod message;

use message::{ServerMessage, ClientMessage};

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
        let message = connection.receive().unwrap();
        sender.send(ClientEvent::ServerMessageReceived(message)).unwrap();
    }
}

enum ClientEvent {
    Key(Key),
    ServerMessageReceived(message::ServerMessage),
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

    fn receive(&mut self) -> std::io::Result<message::ServerMessage> {
        loop {
            let mut conn = self.connection.lock().unwrap();
            use message::ReadServerMessage;
            match conn.read_message() {
                Ok(message) => {
                    debug!("Received message: {:?}", &message);
                    return Ok(message);
                },
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_secs(0)),
                Err(e) => {
                    error!("Error when reading socket: {e:?}");
                    return Err(e);
                },
            }
        }
    }
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
    let server_message_receiver = sender;
    let connection2 = connection.clone();
    std::thread::spawn(move || keyboard_input_thread(keyboard_input_thread_receiver));
    std::thread::spawn(move || server_message_received_thread(connection2, server_message_receiver));

    clear_screen(&mut stdout);
    connection.send(message::ClientMessage::Connect).unwrap();

    'runloop: loop {
        let event = receiver.recv().unwrap();
        match event {
            ClientEvent::Key(key) => {
                if key == Key::Ctrl('c') {
                    info!("Detected C-c. Exiting.");
                    connection.send(message::ClientMessage::Disconnect).unwrap();
                    break 'runloop;
                } else if key == Key::Ctrl('r') {
                    info!("Requesting refresh");
                    connection.send(message::ClientMessage::RequestRefresh).unwrap();
                }

                stdout.flush().unwrap();
            },
            ClientEvent::ServerMessageReceived(message) => {
                info!("Received message: {message:?}");
                match message {
                    ServerMessage::Update(pos, ch) => {
                        write!(
                            stdout,
                            "{}{ch}",
                            termion::cursor::Goto(pos.0 + 1, pos.1 + 1),
                        ).unwrap();
                        stdout.flush().unwrap();
                    },
                    _ => (),
                }
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
