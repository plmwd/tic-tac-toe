use std::collections::HashMap;

use bytes::BytesMut;
use ron;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufWriter};
use tokio::net::TcpListener;
use tokio::sync::Mutex;

// =IDEAS
// - ssh server so others can play in cli without needing to install

type Token = u32;

#[derive(Debug, Serialize, Deserialize)]
enum Notification {}

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

struct Player {
    name: String,
}

#[derive(Default)]
struct State {
    players: HashMap<Token, Player>,
    next_player_id: Token,
    messages: Vec<(String, String)>,
}

// echo 'Text("hello there")' | nc localhost 6969
pub async fn run(listener: TcpListener) {
    println!("listening for connections...");
    let state = Arc::new(Mutex::new(State::default()));

    loop {
        let (socket, addr) = listener.accept().await.unwrap();
        println!("got connection: {}", addr);
        let mut stream = BufWriter::new(socket);
        let mut buffer = String::with_capacity(256);
        let state = Arc::clone(&state);

        tokio::spawn(async move {
            stream.read_to_string(&mut buffer).await.unwrap();
            let maybe_req: Result<Request, _> = ron::from_str(&buffer);
            println!("got {:#?}", maybe_req);

            let rsp: Response = match maybe_req {
                Err(_) => Response::Err("invalid request".to_string()),
                Ok(req) => match req {
                    Request::Ping => Response::Ping,
                    Request::Join { name } => {
                        let mut state = state.lock().await;
                        let token = state.next_player_id;
                        state.next_player_id += 1;
                        state.players.insert(token, Player { name });
                        Response::Joined(token)
                    }
                    Request::Chat { token, message } => {
                        let mut state = state.lock().await;
                        match state.players.get(&token) {
                            None => Response::Err("no user found".to_string()),
                            Some(player) => {
                                let name = player.name.clone();
                                state.messages.push((name, message));
                                Response::Ok
                            }
                        }
                    }
                },
            };

            // let rsp = ron::to_string(&mes).unwrap();
            stream
                .write_all(ron::to_string(&rsp).unwrap().as_bytes())
                .await
                .unwrap();
            stream.flush().await.unwrap();
        });
    }
}
