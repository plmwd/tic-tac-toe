use crate::game::{Conclusion, Game, Player};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    Request(Request),
    Response(Result<Response, Error>),
    Notification(Notification),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Request {
    JoinMatch(Option<Player>),
    GetGameInfo,
    Chat(String),
    PlayTurn(u8),
    Disconnect,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Response {
    Ack,
    GameInfo(Game),
    Joined(Option<Player>),
    TurnDone(Game),
    GameConcluded(Conclusion),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Notification {
    Chat { from: String, msg: String },
    ServerInfo(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Error {
    WaitingForHost,
    InvalidTile,
    NotYourTurn,
    NotAllowed,
    MatchInProgress,
    GameConcluded(Conclusion),
    InvalidParam(String),
    InvalidMessage(String),
    ServerError(String),
}

// TODO: maybe experiment with macros to do this
impl From<Request> for Message {
    fn from(value: Request) -> Self {
        Message::Request(value)
    }
}

impl From<Notification> for Message {
    fn from(value: Notification) -> Self {
        Message::Notification(value)
    }
}

impl From<Response> for Message {
    fn from(value: Response) -> Self {
        Message::Response(Ok(value))
    }
}

impl From<Error> for Message {
    fn from(value: Error) -> Self {
        Message::Response(Err(value))
    }
}
