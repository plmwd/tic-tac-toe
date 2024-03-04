use std::{net::SocketAddr, sync::Arc};

use bytes::{BufMut, BytesMut};
use serde::{Deserialize, Serialize};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter},
    net::{TcpListener, TcpStream},
    select,
    sync::{broadcast, mpsc, oneshot, Mutex},
};

use crate::game::{self, Conclusion, Game};

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

    async fn read_message(&mut self) -> anyhow::Result<Option<Message>> {
        self.buffer.clear();
        loop {
            match ron::de::from_bytes(&self.buffer) {
                Ok(mes) => return Ok(Some(mes)),
                Err(ron::error::SpannedError { code, .. }) => match code {
                    ron::Error::ExpectedDifferentLength { .. } => {}
                    ron::Error::Eof => {}
                    e => {
                        println!("error reading message {:#?}", e);
                        self.write_message(&Message::InvalidMessage(format!("{}", e)))
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

async fn handle_connection(mut con: Connection, server: ServerHandle) -> anyhow::Result<()> {
    let mut cx = ConnectionContext::Observer;

    loop {
        match con.read_message().await.unwrap() {
            Some(Message::Disconnect) => {
                println!("disconnect");
                break;
            }
            None => {
                println!("con EOF");
                break;
            }
            Some(mes) => {
                println!("got mes: {:?}", mes);
                let (tx, rx) = oneshot::channel();
                server.req_tx.send((cx, mes, tx)).await?;
                let (new_cx, rsp) = rx.await?;
                cx = new_cx;
                con.write_message(&rsp).await?;
            }
        };
    }

    let (tx, _) = oneshot::channel();
    server.req_tx.send((cx, Message::Disconnect, tx)).await?;
    Ok(())
}

#[derive(Debug, Copy, Clone, PartialEq)]
enum ConnectionContext {
    Observer,
    Player(game::Player),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum Message {
    Join,
    Joined(Option<game::Player>),
    Disconnect,
    GetInfo,
    Info(Game),
    Chat(String),
    PlayTurn(u8),
    InvalidTile,
    NotYourTurn,
    InvalidMessage(String),
}

type Request = (
    ConnectionContext,
    Message,
    oneshot::Sender<(ConnectionContext, Message)>,
);

// Game flow:
//  - wait for two connections
//      - while waiting, disallow turns but allow chat
// - the two connections play
//      - if any connection is lost, the other player automatically wins
// - additional connections will watch the match
struct Server {
    broadcast: broadcast::Sender<Message>,
    req_rx: mpsc::Receiver<Request>,
    req_tx: mpsc::Sender<Request>,
    num_connections: u32,
}

struct ServerHandle {
    req_tx: mpsc::Sender<Request>,
    broadcast: broadcast::Receiver<Message>,
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

    fn handle_request(&mut self, (cx, msg, rsp): Request) {
        use game::Player::{O, X};
        use ConnectionContext::{Observer, Player};

        let r: (ConnectionContext, Message) = match msg {
            Message::Join if Observer == cx => {
                let cm = match self.num_connections {
                    0 => (Player(X), Message::Joined(Some(X))),
                    1 => (Player(O), Message::Joined(Some(O))),
                    _ => (Observer, Message::Joined(None)),
                };
                self.num_connections += 1;
                cm
            }
            msg => (cx, msg),
        };

        _ = rsp.send(r);
    }

    pub async fn run(mut self) -> anyhow::Result<()> {
        let listener = TcpListener::bind("127.0.0.1:6969").await.unwrap();
        println!("listening on {}...", listener.local_addr()?);

        #[derive(Debug)]
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
                Action::Request(Some(req)) => self.handle_request(req),
                Action::Request(None) => {
                    println!("lost connection");
                }
            };
        }
    }
}

pub async fn run() {
    _ = Server::new().run().await;
}
