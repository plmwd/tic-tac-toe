use std::{collections::HashMap, net::SocketAddr};

use bytes::BytesMut;
use serde::{Deserialize, Serialize};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
    select,
    sync::{broadcast, mpsc, oneshot},
    task::{AbortHandle, JoinSet},
};

use crate::game;

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

    async fn recv(&mut self) -> anyhow::Result<Option<Message>> {
        self.buffer.clear();
        loop {
            match ron::de::from_bytes(&self.buffer) {
                Ok(mes) => return Ok(Some(mes)),
                Err(ron::error::SpannedError { code, .. }) => match code {
                    ron::Error::ExpectedDifferentLength { .. } => {}
                    ron::Error::Eof => {}
                    e => {
                        println!("error reading message {:#?}", e);
                        self.send(&Message::Response(Err(ErrorResponse::InvalidMessage(
                            format!("{}", e),
                        ))))
                        .await
                        .unwrap();
                        self.buffer.clear();
                    }
                },
            }

            if 0 == self.stream.read_buf(&mut self.buffer).await? {
                if self.buffer.is_empty() {
                    return Ok(None);
                }
                anyhow::bail!("Connection reset by peer");
            }
        }
    }

    async fn send(&mut self, mes: &Message) -> tokio::io::Result<()> {
        self.stream
            .write_all(format!("{}\n", ron::ser::to_string(&mes).unwrap()).as_bytes())
            .await?;
        self.stream.flush().await?;
        Ok(())
    }
}

async fn handle_connection(mut con: Connection, mut server: ServerHandle) -> anyhow::Result<()> {
    loop {
        match con.recv().await.unwrap() {
            None => {
                println!("con EOF");
                break;
            }
            Some(Message::Disconnect) => {
                println!("disconnect");
                break;
            }
            Some(Message::Request(req)) => {
                println!("got request: {:?}", req);
                let rsp = server.request(req).await;
                con.send(&Message::Response(rsp)).await?;
            }
            Some(_) => {
                con.send(&Message::Response(Err(ErrorResponse::InvalidMessage(
                    "not a request".to_string(),
                ))))
                .await?
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
    Disconnect,
    Request(Request),
    // TODO: flatten this so that Response has Error variant
    Response(Result<Response, ErrorResponse>),
    Notification(Notification),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum Request {
    Join,
    GetGameInfo,
    Chat(String),
    PlayTurn(u8),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum Response {
    Ok,
    GameInfo(game::Game),
    Joined(Option<game::Player>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum Notification {
    Chat { from: String, msg: String },
    ServerInfo(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum ErrorResponse {
    InvalidTile,
    NotYourTurn,
    InvalidMessage(String),
    ServerError(String),
}

type ContextedRequest = (
    ConnectionId,
    Request,
    oneshot::Sender<Result<Response, ErrorResponse>>,
);

#[derive(Debug)]
struct ServerHandle {
    conn_id: ConnectionId,
    req_tx: mpsc::Sender<ContextedRequest>,
    broadcast: broadcast::Receiver<Notification>,
}

impl ServerHandle {
    async fn request(&mut self, req: Request) -> Result<Response, ErrorResponse> {
        let (tx, rx) = oneshot::channel();
        self.req_tx.send((self.conn_id, req, tx)).await.unwrap();
        rx.await.unwrap()
    }
}

#[derive(Debug)]
enum ServerState {
    WaitingForPlayers,
    Playing(game::Game),
}

// Game flow:
//  - wait for two connections
//      - while waiting, disallow turns but allow chat
// - the two connections play
//      - if any connection is lost, the other player automatically wins
// - additional connections will watch the match
#[derive(Debug)]
struct Server {
    broadcast: broadcast::Sender<Notification>,
    req_rx: mpsc::Receiver<ContextedRequest>,
    req_tx: mpsc::Sender<ContextedRequest>,
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
            Request(Option<ContextedRequest>),
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
            println!("post processing {:#?}", &self);
        }
    }

    fn handle_request(&mut self, (conn_id, req, rsp): ContextedRequest) {
        let Some(cx) = self.contexts.get_mut(&conn_id) else {
            println!("dropping request {} {:#?}", conn_id, req);
            return;
        };

        let r: Result<Response, ErrorResponse> = match (&self.state, req) {
            (_, Request::Chat(msg)) => {
                let from = match cx.group {
                    Group::Observer => cx.addr.to_string(),
                    Group::Player(p) => p.to_string(),
                };
                self.broadcast
                    .send(Notification::Chat { from, msg })
                    .unwrap();
                Ok(Response::Ok)
            }
            (ServerState::WaitingForPlayers, _) => Err(ErrorResponse::InvalidMessage(
                "game not in progress".to_string(),
            )),
            _ => Err(ErrorResponse::InvalidMessage("not implemented".to_string())),
        };

        rsp.send(r).unwrap();
    }

    fn handle_disconnect(&mut self, conn_id: ConnectionId) {
        let cx = self
            .contexts
            .remove(&conn_id)
            .expect("connections cannot be removed twice");
        println!("client disconnected {:#?}", cx);
        let _ = self.broadcast.send(Notification::ServerInfo(format!(
            "{} disconnected",
            cx.addr
        )));
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
