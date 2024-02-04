use bytes::{BufMut, BytesMut};
use serde::{Deserialize, Serialize};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter},
    net::{TcpListener, TcpStream},
};

use crate::game::{Conclusion, Game, Player};

#[derive(Debug, Serialize, Deserialize)]
enum Message {
    Error(String),
    Chat(String),
    Session {
        game: Game,
        you_are: Player,
        turn: Player,
    },
    MarkTile(u8),
}

struct Connection {
    stream: BufReader<TcpStream>,
    buffer: BytesMut,
}

impl Connection {
    fn new(socket: TcpStream) -> Self {
        Self {
            buffer: BytesMut::with_capacity(256),
            stream: BufReader::new(socket),
        }
    }

    async fn read_message(&mut self) -> anyhow::Result<Option<Message>> {
        self.buffer.clear();
        loop {
            match ron::de::from_bytes(&self.buffer) {
                Ok(mes) => return Ok(Some(mes)),
                Err(ron::error::SpannedError { code, .. }) => match code {
                    ron::Error::ExpectedDifferentLength { .. } => {}
                    ron::Error::Eof => {}
                    e => {
                        println!("error reading message {:?}", e);
                        self.write_message(&Message::Error(format!("{}", e)))
                            .await
                            .unwrap();
                        self.buffer.clear();
                    }
                },
            }

            if 0 == self.stream.read_buf(&mut self.buffer).await? {
                if self.buffer.is_empty() {
                    return Ok(None);
                } else {
                    anyhow::bail!("Connection reset by peer");
                }
            }
        }
    }

    async fn write_message(&mut self, mes: &Message) -> tokio::io::Result<()> {
        self.stream
            .write_all(format!("{}\n", ron::ser::to_string(&mes).unwrap()).as_bytes())
            .await?;
        self.stream.flush().await?;
        Ok(())
    }
}

async fn handle_connection(mut con: Connection) -> anyhow::Result<()> {
    loop {
        match con.read_message().await.unwrap() {
            Some(mes) => {
                println!("got mes: {:?}", mes);
                con.write_message(&mes).await.unwrap();
            }
            None => {
                println!("con EOF");
            }
        };
    }
}

pub async fn run(listener: TcpListener) -> anyhow::Result<()> {
    loop {
        let (socket, addr) = listener.accept().await.unwrap();
        println!("got new connection {:?}", addr);
        let con = Connection::new(socket);

        tokio::spawn(async move { handle_connection(con).await });
    }
}
