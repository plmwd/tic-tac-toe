use std::{
    collections::{
        hash_map::{Entry, OccupiedEntry},
        HashMap,
    },
    net::SocketAddr,
};

use tokio::{
    net::{TcpListener, TcpStream},
    select,
    sync::{broadcast, mpsc, oneshot},
    task::{AbortHandle, JoinSet},
};

use crate::{
    connection::Connection,
    message::{Error as ErrorResponse, Message, Notification, Request, Response},
};
use crate::{connection::ConnectionId, game};

async fn handle_connection(mut con: Connection, mut server: ServerHandle) -> anyhow::Result<()> {
    use broadcast::error::RecvError;

    loop {
        select! {
            notification = server.broadcast.recv() => {
                match notification {
                    Ok(notification) => {
                        con.send(notification).await?;
                    },
                    Err(RecvError::Lagged(num_skipped)) => {
                        println!("connection {} lagged by {} notifications", con.addr, num_skipped);
                        continue;
                    },
                    Err(RecvError::Closed) => anyhow::bail!("server broadcast dropped"),
                }
            }
            msg = con.recv() => {
                match msg? {
                    None => {
                        println!("con EOF");
                        break;
                    }
                    Some(Request::Disconnect) => {
                        println!("disconnect");
                        break;
                    }
                    Some(req) => {
                        println!("got request: {:?}", req);
                        let rsp = server.request(req).await;
                        con.send(Message::Response(rsp)).await?;
                    }
                };
            }
        }
    }

    Ok(())
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum Group {
    Host(Option<game::Player>),
    Observer,
    Player(game::Player),
}

#[derive(Debug)]
struct ConnectionContext {
    group: Group,
    addr: SocketAddr,
    abort_handle: AbortHandle,
}

impl ConnectionContext {
    fn is_host(&self) -> bool {
        matches!(self.group, Group::Host(_))
    }

    fn player(&self) -> Option<game::Player> {
        match self.group {
            Group::Observer => None,
            Group::Host(p) => p,
            Group::Player(p) => Some(p),
        }
    }
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

#[derive(Default, Debug)]
enum ServerState {
    #[default]
    WaitingForHost,
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
            state: Default::default(),
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
        use ErrorResponse::{InvalidMessage, InvalidParam, MatchInProgress, WaitingForHost};
        use Request::{Chat, GetGameInfo, JoinMatch, PlayTurn};
        use Response::{Ack, Joined};

        let Entry::Occupied(cx) = self.contexts.entry(conn_id) else {
            println!("dropping request {} {:#?}", conn_id, req);
            return;
        };

        let r: Result<Response, ErrorResponse> = match (req, &self.state) {
            (Chat(msg), _) => {
                let cx = cx.get();
                let from = match cx.group {
                    Group::Observer => cx.addr.to_string(),
                    Group::Player(p) => p.to_string(),
                    Group::Host(None) => "host".to_string(),
                    Group::Host(Some(p)) => format!("{p} (host)"),
                };
                self.broadcast
                    .send(Notification::Chat { from, msg })
                    .unwrap();
                Ok(Ack)
            }

            (JoinMatch(player), ServerState::WaitingForHost) if cx.get().is_host() => {
                cx.into_mut().group = Group::Host(player);
                self.state = ServerState::WaitingForPlayers;
                Ok(Joined(player))
            }
            (JoinMatch(req_join_as), ServerState::WaitingForPlayers) => {
                // Once the second player joins, the state is moved to Playing
                match cx.get().group {
                    group @ (Group::Observer | Group::Host(None)) => {
                        // Find existing player, if any
                        let already_joined =
                            self.contexts.iter().find_map(|id_cx| match id_cx.1.group {
                                Group::Host(Some(other)) | Group::Player(other) => Some(other),
                                _ => None,
                            });

                        let join_as = match (req_join_as, already_joined) {
                            (None, None) => game::Player::O,
                            (Some(req_join_as), None) => req_join_as,
                            (None | Some(_), Some(already_joined)) => !already_joined,
                        };

                        let new_group = if matches!(group, Group::Observer) {
                            Group::Player(join_as)
                        } else {
                            Group::Host(Some(join_as))
                        };

                        self.contexts
                            .entry(conn_id)
                            .and_modify(|cx| cx.group = new_group);

                        if let Some(_) = already_joined {
                            self.state = ServerState::Playing(game::Game::new(game::Player::O))
                        }

                        Ok(Response::Joined(Some(join_as)))
                    }
                    Group::Player(_) | Group::Host(Some(_)) => {
                        Err(InvalidParam("already joined".to_string()))
                    }
                }
            }
            (GetGameInfo, ServerState::Playing(game)) => Ok(Response::GameInfo(game.clone())),
            (PlayTurn(tile), ServerState::Playing(game)) => todo!(),
            (PlayTurn(_), _) => Err(ErrorResponse::NotAllowed),
            (GetGameInfo, _) => Err(ErrorResponse::NotAllowed),
            (JoinMatch(_), ServerState::Playing(_)) => Err(ErrorResponse::MatchInProgress),
            (_, ServerState::WaitingForHost) => Err(ErrorResponse::WaitingForHost),
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

        let con = Connection::new(socket, addr);
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

        let group = if addr.ip().is_loopback() {
            Group::Host(None)
        } else {
            Group::Observer
        };

        self.contexts.insert(
            conn_id,
            ConnectionContext {
                group,
                addr,
                abort_handle,
            },
        );
    }
}

pub async fn run() {
    _ = Server::default().run().await;
}
