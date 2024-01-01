mod board;
mod game;
use board::Player;
use game::play;

fn main() {
    play(Player::O);
}
