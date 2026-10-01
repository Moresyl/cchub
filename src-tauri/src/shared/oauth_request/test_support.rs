use crate::shared::usage_http::test_support::read_headers;
use tokio::io::AsyncWriteExt;
use tokio::sync::oneshot;

pub(crate) async fn gated_rejection() -> (
    String,
    oneshot::Receiver<()>,
    oneshot::Sender<()>,
    tokio::task::JoinHandle<Vec<String>>,
) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/resource", listener.local_addr().unwrap());
    let (seen, observed) = oneshot::channel();
    let (release, released) = oneshot::channel();
    let task = tokio::spawn(async move {
        let (mut first, _) = listener.accept().await.unwrap();
        let one = read_headers(&mut first).await;
        seen.send(()).unwrap();
        released.await.unwrap();
        first
            .write_all(
                b"HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            )
            .await
            .unwrap();
        first.shutdown().await.unwrap();
        let (mut second, _) = listener.accept().await.unwrap();
        let two = read_headers(&mut second).await;
        second.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 15\r\nConnection: close\r\n\r\n{\"result\":\"ok\"}").await.unwrap();
        second.shutdown().await.unwrap();
        vec![one, two]
    });
    (url, observed, release, task)
}
