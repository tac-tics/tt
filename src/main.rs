use interprocess::local_socket::{LocalSocketListener, LocalSocketStream};
use signal_hook::{consts::SIGTERM, iterator::Signals};
use std::io::{Write, Read};

pub mod message;

const IPC_DIR: &'static str = "/home/tac-tics/projects/tt/ipc";

fn main() {
    // let pid_file = &format!("{IPC_DIR}/tt.pid");
    let args: Vec<String> = std::env::args().collect();

    let name = format!("{IPC_DIR}/tt.sock");
    let sock_path = std::path::Path::new(&name);
    if !sock_path.exists() {
        eprintln!("tt-daemon isn't running");
        std::process::exit(1);
    }

    use message::WriteClientMessage;
    let mut conn = LocalSocketStream::connect(sock_path).unwrap();
    conn.write_message(message::ClientMessage::Resize((10, 10))).unwrap();
}
