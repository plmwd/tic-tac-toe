mod board;
mod game;
use board::Player;
use game::{play, Game};

fn main() {
    let game = Game::new(Player::X);
    play(game);
}
