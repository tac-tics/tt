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

pub mod message;
use message::{ClientMessage, ServerMessage};

const IPC_DIR: &'static str = "/home/tac-tics/projects/tt/ipc";

#[derive(Default, Serialize, Deserialize)]
pub struct TermTextState {
    pub data: String,
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

    fn receive(&mut self) -> std::io::Result<message::ClientMessage> {
        loop {
            let mut conn = self.connection.lock().unwrap();
            use message::ReadClientMessage;
            match conn.read_message() {
                Ok(message) => {
                    debug!("Received message: {:?}", &message);
                    return Ok(message);
                },
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_secs(0)),
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => std::thread::sleep(Duration::from_secs(0)),
                Err(e) => {
                    error!("Error when reading socket: {e:?}");
                    return Err(e);
                },
            }
        }
    }
}

#[derive(Debug)]
enum ServerEvent {
    ClientMessageReceived(message::ClientMessage),
    OpenFile(String),
}

fn client_message_received_thread(mut connection: Connection, sender: mpsc::Sender<ServerEvent>) {
    loop {
        let message = connection.receive().unwrap();
        sender.send(ServerEvent::ClientMessageReceived(message)).unwrap();
    }
}

fn send_update(connection: &mut Connection) -> anyhow::Result<()> {
    info!("called send_update");
    let mut pos = (0, 0);
    let data = {
        if let Ok(termtext) = TERMTEXT.lock() {
            termtext.data.clone()
        } else {
            error!("UH OH");
            panic!()
        }
    };
    info!("Trying to send {data}");

    for ch in data.chars() {
        if ch == '\n' {
            pos = (0, pos.1 + 1);
        } else if ch == '\t' {
            pos.0 = pos.0 + 4;
        } else {
            info!("About to send it... ");
            connection.send(ServerMessage::Update(pos, ch))?;
            std::thread::sleep(Duration::from_millis(1));
            info!("Ok");
            pos.0 = pos.0 + 1;
        }
    }
    Ok(())
}

fn handle_connection(connection: LocalSocketStream) -> anyhow::Result<()> {
    let mut connection = Connection::make(connection);
    let (sender, receiver) = mpsc::channel();

    let connection1 = connection.clone();
    let sender1 = sender.clone();
    std::thread::spawn(|| {
        client_message_received_thread(connection1, sender1);
    });

    let sender2 = sender.clone();
    std::thread::spawn(move || {
        loop {
            let path = format!("{IPC_DIR}/hello.txt");
            sender2.send(ServerEvent::OpenFile(path)).unwrap();
            std::thread::sleep(Duration::from_millis(1));

            let path = format!("{IPC_DIR}/world.txt");
            sender2.send(ServerEvent::OpenFile(path)).unwrap();
            std::thread::sleep(Duration::from_millis(1));
        }
    });

    'serve_loop: loop {
        info!("About to get next message");
        let message = receiver.recv()?;
        info!("{:?}", &message);
        match &message {
            ServerEvent::ClientMessageReceived(message) => {
                info!("Received message: {message:?}");
                match message {
                    ClientMessage::Connect => {
                        send_update(&mut connection)?;
                    },
                    ClientMessage::RequestRefresh => {
                        send_update(&mut connection)?;
                    },
                    ClientMessage::Disconnect => break 'serve_loop,
                    _ => info!("{:?}", &message),
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
                send_update(&mut connection)?;
            },
        }
    }
    Ok(())
}
