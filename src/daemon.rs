use std::sync::{Arc, Mutex};
use interprocess::local_socket::{LocalSocketListener, LocalSocketStream};
use signal_hook::{consts::SIGTERM, iterator::Signals};
use std::io::{Write, Read};
use simplelog::*;
use anyhow;
use log::{info, warn, error};
use lazy_static::lazy_static;
use serde::{Serialize, Deserialize};

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
        LevelFilter::Info,
        Config::default(),
        std::fs::File::create(format!("{IPC_DIR}/tt.log")).unwrap(),
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

fn service_connection(mut connection: LocalSocketStream) -> anyhow::Result<()> {
    use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
    {
        info!("Sending state");
        let tt = TERMTEXT.lock().unwrap();
        connection.write_u64::<LittleEndian>(tt.data.len().try_into()?)?;
        connection.write(tt.data.as_bytes())?;
        connection.flush()?;
        info!("State sent");
    }
    let mut buffer: Vec<u8> = vec![0; 4096];
    info!("Reading data");
    {
        let size: usize = connection.read_u64::<LittleEndian>()?.try_into()?;
        connection.read(&mut buffer[..size])?;
        info!("Read data: {:?}", String::from_utf8_lossy(&buffer[..size]));
        info!("Updating state");
        let mut tt = TERMTEXT.lock().unwrap();
        info!("Got the lock");
        tt.data = String::from_utf8_lossy(&buffer[..size]).to_string();
        info!("Updating state: {:?}", &tt.data);
    }
    Ok(())
}
