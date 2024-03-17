#![allow(dead_code)]

mod board;
mod connection;
mod game;
mod message;
mod server;
mod term;
use tokio;

#[tokio::main]
async fn main() {
    server::run().await;
}
