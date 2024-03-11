use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use bytes::{BufMut, BytesMut};
use serde::{Deserialize, Serialize};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter},
    net::{TcpListener, TcpStream},
    select,
    sync::{broadcast, mpsc, oneshot, Mutex},
    task::{AbortHandle, JoinSet},
};

use crate::game::{self, Game};

type ConnectionId = u32;

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
            }
        };
    }

    Ok(())
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum Group {
    Observer,
    Player(game::Player),
}

#[derive(Debug)]
struct ConnectionContext {
    group: Group,
    addr: SocketAddr,
    abort_handle: AbortHandle,
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

type Request = (ConnectionId, Message, oneshot::Sender<Message>);

#[derive(Debug)]
struct ServerHandle {
    conn_id: ConnectionId,
    req_tx: mpsc::Sender<Request>,
    broadcast: broadcast::Receiver<Message>,
}

impl ServerHandle {
    async fn request(&mut self, msg: Message) -> Message {
        let (tx, rx) = oneshot::channel();
        self.req_tx.send((self.conn_id, msg, tx)).await.unwrap();
        rx.await.unwrap()
    }
}

enum ServerState {
    WaitingForPlayers,
    Playing(Game),
}

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
    contexts: HashMap<ConnectionId, ConnectionContext>,
    connections: JoinSet<ConnectionId>,
    next_conn_id: ConnectionId,
    state: ServerState,
}

impl Default for Server {
    fn default() -> Self {
        let (req_tx, req_rx) = mpsc::channel(32);
        let (broadcast, _) = broadcast::channel(32);
        Self {
            broadcast,
            req_rx,
            req_tx,
            contexts: HashMap::with_capacity(32),
            connections: JoinSet::new(),
            next_conn_id: 0,
            state: ServerState::WaitingForPlayers,
        }
    }
}

impl Server {
    pub async fn run(mut self) -> anyhow::Result<()> {
        let listener = TcpListener::bind("127.0.0.1:6969").await.unwrap();
        println!("listening on {}...", listener.local_addr()?);

        #[derive(Debug)]
        enum Action {
            NewConnection(TcpStream, SocketAddr),
            Disconnected(ConnectionId),
            Request(Option<Request>),
        }

        loop {
            let action = select! {
                con = listener.accept() => con.map(|(socket, addr)| Action::NewConnection(socket, addr)).unwrap(),
                req = self.req_rx.recv() => Action::Request(req),
                maybe_join = self.connections.join_next() => {
                    match maybe_join {
                        // TODO: not handling panics or cancellation
                        // How to associate id to task without using unstable-tokio?
                        Some(conn_id) => {
                            println!("joined {:#?}", conn_id);
                            Action::Disconnected(conn_id.unwrap())},
                        None => continue
                    }
                }
            };

            println!("processing {:#?}", action);
            match action {
                Action::NewConnection(socket, addr) => self.handle_new_connection(socket, addr),
                Action::Request(Some(req)) => self.handle_request(req),
                Action::Disconnected(conn_id) => self.handle_disconnect(conn_id),
                Action::Request(None) => {
                    panic!("unknown error handling requests");
                }
            };
        }
    }

    fn handle_request(&mut self, (cx, msg, rsp): Request) {
        todo!()
    }

    fn handle_disconnect(&mut self, conn_id: ConnectionId) {
        let cx = self
            .contexts
            .remove(&conn_id)
            .expect("connections cannot be removed twice");
        println!("client disconnected {:#?}", cx);
        // TODO: send notification?
    }

    fn handle_new_connection(&mut self, socket: TcpStream, addr: SocketAddr) {
        let conn_id = self.next_conn_id;
        self.next_conn_id += 1;
        assert!(
            !self.contexts.contains_key(&conn_id),
            "connection id already in use"
        );

        let con = Connection::new(socket);
        let handle = ServerHandle {
            req_tx: self.req_tx.clone(),
            broadcast: self.broadcast.subscribe(),
            conn_id,
        };

        let abort_handle = self.connections.spawn(async move {
            let conn_id = handle.conn_id;
            println!(
                "connection closed {:#?}",
                handle_connection(con, handle).await
            );
            conn_id
        });

        self.contexts.insert(
            conn_id,
            ConnectionContext {
                group: Group::Observer,
                addr,
                abort_handle,
            },
        );
    }
}

pub async fn run() {
    _ = Server::default().run().await;
}
