#[derive(Debug, Default, PartialEq, Clone, Copy)]
enum Player {
    X,
    #[default]
    O,
}

#[derive(Debug, Default, PartialEq, Clone)]
struct Board {
    tiles: [Option<Player>; 9],
}

#[derive(Debug, PartialEq, Clone, Copy)]
enum GameConclusion {
    Win(Player),
    Draw,
}

#[derive(Default)]
struct GameState {
    board: Board,
    turn: Player,
}

enum Game {
    Playing(GameState),
    Concluded(GameConclusion),
}

impl GameState {
    fn play(self, new_mark_pos: (u8, u8)) -> Game {
        // how to handle invalid input?
        Game::Playing(self)
    }
}

impl Game {
    fn new(first_turn: Player) -> Self {
        Self::Playing(GameState {
            turn: first_turn,
            board: Board::default(),
        })
    }
}

fn main() {
    println!("Hello, world!");
}
