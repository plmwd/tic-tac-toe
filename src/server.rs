use std::{net::SocketAddr, sync::Arc};

use bytes::{BufMut, BytesMut};
use serde::{Deserialize, Serialize};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter},
    net::{TcpListener, TcpStream},
    select,
    sync::{broadcast, mpsc, oneshot, Mutex},
};

use crate::game::{Conclusion, Game, Player};

#[derive(Debug, Clone, Serialize, Deserialize)]
enum ClientMessage {
    Error(String),
    Chat(String),
    Game { game: Game, turn: Player },
    MarkTile(u8),
}

#[derive(Debug)]
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

    async fn read_message(&mut self) -> anyhow::Result<Option<ClientMessage>> {
        self.buffer.clear();
        loop {
            match ron::de::from_bytes(&self.buffer) {
                Ok(mes) => return Ok(Some(mes)),
                Err(ron::error::SpannedError { code, .. }) => match code {
                    ron::Error::ExpectedDifferentLength { .. } => {}
                    ron::Error::Eof => {}
                    e => {
                        println!("error reading message {:?}", e);
                        self.write_message(&ClientMessage::Error(format!("{}", e)))
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

    async fn write_message(&mut self, mes: &ClientMessage) -> tokio::io::Result<()> {
        self.stream
            .write_all(format!("{}\n", ron::ser::to_string(&mes).unwrap()).as_bytes())
            .await?;
        self.stream.flush().await?;
        Ok(())
    }
}

async fn handle_connection(mut con: Connection, mut server: ServerHandle) -> anyhow::Result<()> {
    loop {
        if let Some(mes) = con.read_message().await.unwrap() {
            println!("got mes: {:?}", mes);
            con.write_message(&mes).await.unwrap();
        } else {
            println!("con EOF");
            break;
        };
    }

    Ok(())
}

#[derive(Debug)]
enum Command {
    Join,
    GetInfo,
    Chat(Player, String),
    PlayTurn(Player, u8),
}

enum CommandResponse {
    Joined(Player),
    Info(Game),
    InvalidTile,
    NotYourTurn,
}

type Request = (Command, oneshot::Sender<CommandResponse>);

// Game flow:
//  - wait for two connections
//      - while waiting, disallow turns but allow chat
// - the two connections play
//      - if any connection is lost, the other player automatically wins
// - additional connections will watch the match
struct Server {
    broadcast: broadcast::Sender<ClientMessage>,
    req_rx: mpsc::Receiver<Request>,
    req_tx: mpsc::Sender<Request>,
    num_connections: u32,
}

struct ServerHandle {
    req_tx: mpsc::Sender<Request>,
    broadcast: broadcast::Receiver<ClientMessage>,
}

impl Server {
    pub fn new() -> Self {
        let (req_tx, req_rx) = mpsc::channel(32);
        let (broadcast, _) = broadcast::channel(32);
        Self {
            broadcast,
            req_rx,
            req_tx,
            num_connections: 0,
        }
    }

    fn handle(&mut self) -> ServerHandle {
        ServerHandle {
            req_tx: self.req_tx.clone(),
            broadcast: self.broadcast.subscribe(),
        }
    }

    pub async fn run(mut self) -> anyhow::Result<()> {
        let listener = TcpListener::bind("127.0.0.1:6969").await.unwrap();

        enum Action {
            NewConnection(TcpStream, SocketAddr),
            Request(Option<Request>),
        }

        loop {
            let action = select! {
                con = listener.accept() => con.map(|(socket, addr)| Action::NewConnection(socket, addr)).unwrap(),
                req = self.req_rx.recv() => Action::Request(req),
            };

            match action {
                Action::NewConnection(socket, addr) => {
                    println!("got new connection {:?}", addr);
                    let con = Connection::new(socket);

                    let handle = self.handle();
                    tokio::spawn(async move { handle_connection(con, handle).await });
                }
                Action::Request(Some((cmd, rsp_callback))) => {
                    println!("got req {:?}", cmd);
                }
                Action::Request(None) => {
                    println!("lost connection");
                }
            }
        }
    }
}

pub async fn run() {
    Server::new().run().await;
}
