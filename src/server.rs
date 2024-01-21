use bytes::BytesMut;
use ron;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufWriter};
use tokio::net::TcpListener;

#[derive(Debug, Serialize, Deserialize)]
enum Message {
    Ping,
    Text(String),
    DeezNuts,
}

// echo 'Text("hello there")' | nc localhost 6969
pub async fn run(listener: TcpListener) {
    println!("listening for connections...");

    loop {
        let (socket, addr) = listener.accept().await.unwrap();
        println!("got connection: {}", addr);
        let mut stream = BufWriter::new(socket);
        let mut buffer = String::with_capacity(256);

        tokio::spawn(async move {
            stream.read_to_string(&mut buffer).await.unwrap();
            let mes: Message = ron::from_str(&buffer).unwrap();
            println!("got {:#?}", mes);
            let rsp = ron::to_string(&mes).unwrap();
            stream.write_all(rsp.as_bytes()).await.unwrap();
            stream.flush().await.unwrap();
        });
    }
}
