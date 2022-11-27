pub mod connection;
pub mod message;


#[cfg(test)]
pub mod test {
    #[test]
    fn client_can_message_server() {
        let path = std::path::PathBuf::from("test_client_can_message_server.sock");

        let path2 = path.clone();
        let path3 = path.clone();
        let _t1 = std::thread::spawn(move || {
            for connection in crate::connection::Listener::listen(path2).unwrap() {
                let mut connection = connection.unwrap();
                let message: usize = connection.receive().unwrap().unwrap();
                assert_eq!(message, 42);
            }
        });
        std::thread::sleep(std::time::Duration::from_millis(10));
        let _t2 = std::thread::spawn(|| {
            let mut conn = crate::connection::Connection::connect(path).unwrap();
            let message: usize = 42;
            conn.send(message).unwrap();
        });

        std::thread::sleep(std::time::Duration::from_millis(100));
        std::fs::remove_file(path3).unwrap();
    }

    #[test]
    fn server_can_message_client() {
        let path = std::path::PathBuf::from("test_server_can_message_client.sock");

        let path2 = path.clone();
        let path3 = path.clone();
        let _t1 = std::thread::spawn(move || {
            for connection in crate::connection::Listener::listen(path2).unwrap() {
                let mut connection = connection.unwrap();
                let message: usize = connection.receive().unwrap().unwrap();
                assert_eq!(message, 42);
            }
        });
        std::thread::sleep(std::time::Duration::from_millis(10));
        let _t2 = std::thread::spawn(|| {
            let mut conn = crate::connection::Connection::connect(path).unwrap();
            let message: usize = 42;
            conn.send(message).unwrap();
        });
        std::thread::sleep(std::time::Duration::from_millis(100));
        std::fs::remove_file(&path3).unwrap();
    }
}
