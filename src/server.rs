use std::collections::hash_map::Entry;
use std::collections::HashMap;

use bytes::BytesMut;
use ron;
use serde::de::value::BorrowedStrDeserializer;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufWriter};
use tokio::net::{TcpListener, TcpStream};
use tokio::select;
use tokio::sync::{broadcast, mpsc, oneshot, Mutex};

// =IDEAS
// - ssh server so others can play in cli without needing to install

type Token = u32;

#[derive(Debug, Serialize, Deserialize, Clone)]
enum Notification {
    Shutdown,
    PlayerLeft { name: String },
    Chat { name: String, message: String },
}

#[derive(Debug, Serialize, Deserialize)]
enum ServerMessage {
    Notification(Notification),
    Response(Response),
}

#[derive(Debug, Serialize, Deserialize)]
enum Request {
    Ping,
    Join { name: String },
    Chat { token: Token, message: String },
}

#[derive(Debug, Serialize, Deserialize)]
enum Response {
    Err(String),
    Ok,
    Ping,
    Joined(Token),
}

struct Connection {
    stream: BufWriter<TcpStream>,
    buffer: String,
    // rx: broadcast::Receiver<Notification>,
    // tx: mpsc::Sender<(Request, oneshot::Sender<Response>)>,
}

#[derive(Default)]
struct State {
    players: HashMap<Token, String>,
    next_player_id: Token,
    messages: Vec<(String, String)>,
}

// echo 'Text("hello there")' | nc localhost 6969
pub async fn run(listener: TcpListener) {
    let (req_tx, mut req_rx) = mpsc::channel(100);
    let (notify_tx, notify_rx) = broadcast::channel(100);
    drop(notify_rx);

    // Start background task
    let notify_tx_clone = notify_tx.clone();
    tokio::spawn(async move {
        let mut state = State::default();
        println!("started bg task");

        while let Some((req, rsp_tx)) = req_rx.recv().await {
            println!("handling req {:#?}", req);
            let rsp = match req {
                Request::Ping => Response::Ping,
                Request::Join { name } => {
                    println!("player '{name}' joined");
                    let token = state.next_player_id;
                    state.next_player_id += 1;
                    state.players.insert(token, name);
                    Response::Joined(token)
                }
                Request::Chat { token, message } => {
                    if !state.players.contains_key(&token) {
                        Response::Err("no user found".into())
                    } else {
                        println!("sending chat notfication");
                        let name = state.players.get(&token).unwrap().clone();
                        state.messages.push((name.clone(), message.clone()));
                        notify_tx
                            .send(Notification::Chat { name, message })
                            .unwrap();
                        Response::Ok
                    }
                }
            };
            oneshot::Sender::send(rsp_tx, rsp).unwrap();
        }
    });

    // Accept new connections
    println!("listening for connections...");
    let notify_tx = notify_tx_clone;
    loop {
        let (socket, addr) = listener.accept().await.unwrap();
        println!("got connection: {}", addr);
        let con = Connection::new(socket);
        let tx = req_tx.clone();
        let rx = notify_tx.subscribe();

        tokio::spawn(async move {
            handle_player_connection(con, tx, rx).await.unwrap();
        });
    }
}

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type AnyResult<T> = std::result::Result<T, Error>;

impl Connection {
    fn new(socket: TcpStream) -> Self {
        Self {
            stream: BufWriter::new(socket),
            buffer: String::with_capacity(256),
        }
    }

    async fn read_request(&mut self) -> tokio::io::Result<Request> {
        loop {
            self.buffer.clear();
            self.stream.read_to_string(&mut self.buffer).await?;
            let maybe_req: Result<Request, _> = ron::from_str(&self.buffer);
            println!("got {:#?}", maybe_req);

            match maybe_req {
                Ok(req) => return Ok(req),
                Err(_) => {
                    self.write_response(Response::Err("invalid request format".to_string()))
                        .await?
                }
            };
        }
    }

    async fn write_response(&mut self, rsp: Response) -> tokio::io::Result<()> {
        self.stream
            .write_all(
                ron::to_string(&ServerMessage::Response(rsp))
                    .unwrap()
                    .as_bytes(),
            )
            .await?;
        Ok(())
    }

    async fn send_notification(&mut self, notification: Notification) -> tokio::io::Result<()> {
        self.stream
            .write_all(
                ron::to_string(&ServerMessage::Notification(notification))
                    .unwrap()
                    .as_bytes(),
            )
            .await?;
        Ok(())
    }
}

async fn handle_player_connection(
    mut con: Connection,
    tx: mpsc::Sender<(Request, oneshot::Sender<Response>)>,
    mut rx: broadcast::Receiver<Notification>,
) -> AnyResult<()> {
    loop {
        tokio::select! {
            request = con.read_request() => {
                let request = request?;
                let (rsp_tx, rsp_rx) = oneshot::channel();
                tx.send((request, rsp_tx)).await?;
                con.write_response(rsp_rx.await?).await?;
            },
            notification = rx.recv() => {
                let notification = notification?;
                con.send_notification(notification.clone()).await?;

                if let Notification::Shutdown = notification {
                    break;
                }
            }
        }
    }
    Ok(())
}
