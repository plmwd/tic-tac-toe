mod board;
mod game;
mod server;
mod term;
use board::Player;
use tokio;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    server::run().await;
}
