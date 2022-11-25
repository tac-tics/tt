use interprocess::local_socket::{LocalSocketListener, LocalSocketStream};
use signal_hook::{consts::SIGTERM, iterator::Signals};
use std::io::{Write, Read, BufRead};
use std::io::{BufReader, BufWriter};

const IPC_DIR: &'static str = "/home/tac-tics/projects/tt/ipc";

fn main() {
    use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
    // let pid_file = &format!("{IPC_DIR}/tt.pid");
    let args: Vec<String> = std::env::args().collect();

    let name = format!("{IPC_DIR}/tt.sock");
    let sock_path = std::path::Path::new(&name);
    if !sock_path.exists() {
        eprintln!("tt-daemon isn't running");
        std::process::exit(1);
    }

    let mut conn = LocalSocketStream::connect(sock_path).unwrap();

    let mut buffer = vec![0; 4096];
    println!("Reading...");
    let size: usize = conn.read_u64::<LittleEndian>().unwrap().try_into().unwrap();
    conn.read(&mut buffer[..size]).unwrap();
    println!("Read");
    println!("{:?}", String::from_utf8_lossy(&buffer[..size]));

    let s = &args.get(1).cloned().unwrap_or_default();
    conn.write_u64::<LittleEndian>(s.len().try_into().unwrap()).unwrap();
    conn.write(s.as_bytes()).unwrap();
}
