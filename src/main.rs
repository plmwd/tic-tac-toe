mod board;
mod game;
mod server;
mod term;
use board::Player;
use tokio;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    let server = TcpListener::bind("127.0.0.1:6969").await.unwrap();
    server::run(server).await;
    // term::play(Player::O);
}
