use interprocess::local_socket::{LocalSocketListener, LocalSocketStream};
use signal_hook::{consts::SIGTERM, iterator::Signals};
use std::io::{Write, Read, BufRead};
use std::io::{BufReader, BufWriter};

const IPC_DIR: &'static str = "/home/tac-tics/projects/tt/ipc";

fn main() {
    // let pid_file = &format!("{IPC_DIR}/tt.pid");
    //let args: Vec<String> = std::env::args().collect();
    //let cmd = &args[1];

    let name = format!("{IPC_DIR}/tt.sock");
    let sock_path = std::path::Path::new(&name);
    if !sock_path.exists() {
        eprintln!("tt-daemon isn't running");
        std::process::exit(1);
    }

    let mut conn = LocalSocketStream::connect(sock_path).unwrap();
    {
        let writer = std::pin::Pin::new(&mut conn).get_mut();
        writer.write_all(b"Hello, world").unwrap();
        writer.flush().unwrap();
    }

    /*
    let mut buffer: Vec<u8> = Vec::with_capacity(4096);
    conn.read(&mut buffer).unwrap();
    println!("{:?}", String::from_utf8_lossy(&buffer));
    */
}
