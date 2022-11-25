use interprocess::local_socket::{LocalSocketListener, LocalSocketStream};
use signal_hook::{consts::SIGTERM, iterator::Signals};
use std::io::{Write, Read};
use simplelog::*;
use anyhow;
use anyhow::Context;
use log::{info, warn, error};

const IPC_DIR: &'static str = "/home/tac-tics/projects/tt/ipc";

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
        match incomming_connection {
            Ok(connection) => service_connection(connection)
                .unwrap_or_else(|err| error!("Error while servicing connection: {err}")),
            Err(err) => error!("Error while opening incoming connection: {err}"),
        }
    }
}

fn service_connection(mut connection: LocalSocketStream) -> anyhow::Result<()> {
    let mut buffer: Vec<u8> = vec![0; 4096];
    let size = connection.read(&mut buffer)?;
    info!("Read data: {:?}", String::from_utf8_lossy(&buffer[..size]));
    Ok(())
}
