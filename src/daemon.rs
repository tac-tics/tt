use std::sync::{Arc, Mutex};
use interprocess::local_socket::{LocalSocketListener, LocalSocketStream};
use signal_hook::{consts::SIGTERM, iterator::Signals};
use std::io::{Write, Read};
use simplelog::*;
use anyhow;
use log::{info, warn, error, debug};
use lazy_static::lazy_static;
use serde::{Serialize, Deserialize};

pub mod message;

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
            Ok(connection) => service_connection(connection)
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
        let connection = Arc::new(Mutex::new(connection));
        Connection { connection }
    }

    fn send(&mut self, message: message::ServerMessage) -> std::io::Result<()> {
        let mut conn = self.connection.lock().unwrap();
        use message::WriteServerMessage;
        debug!("Sending message: {:?}", message);
        conn.write_message(message)
    }

    fn receive(&mut self) -> std::io::Result<message::ClientMessage> {
        let mut conn = self.connection.lock().unwrap();
        use message::ReadClientMessage;
        let message = conn.read_message()?;
        debug!("Received message: {:?}", &message);
        Ok(message)
    }
}

fn service_connection(connection: LocalSocketStream) -> anyhow::Result<()> {
    let mut connection = Connection::make(connection);
    connection.send(message::ServerMessage::Update((0, 0), 'H'))?;
    connection.send(message::ServerMessage::Update((1, 0), 'i'))?;
    connection.send(message::ServerMessage::Update((2, 0), '!'))?;
    loop {
        let message = connection.receive()?;
        info!("{:?}", &message);
        match &message {
            message::ClientMessage::Disconnect => break,
            _ => info!("{:?}", &message),
        }
    }
    Ok(())
}
