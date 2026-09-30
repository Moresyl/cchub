use tokio::{io::AsyncReadExt, net::TcpStream};

pub(crate) async fn read_headers(socket: &mut TcpStream) -> String {
    let mut bytes = Vec::new();
    loop {
        let mut chunk = [0; 1024];
        let count =
            tokio::time::timeout(std::time::Duration::from_secs(2), socket.read(&mut chunk))
                .await
                .expect("request headers timed out")
                .expect("request headers failed");
        assert!(count > 0, "request ended before the complete header block");
        bytes.extend_from_slice(&chunk[..count]);
        assert!(bytes.len() <= 16 * 1024, "unexpectedly large test request");
        if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }
    String::from_utf8(bytes).expect("test request must be UTF-8")
}
