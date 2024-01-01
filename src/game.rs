use crate::board::{Board, Player, TileId};
use std::{
    io::{self, Write},
    str::FromStr,
};

pub fn play(first_turn: Player) {
    let mut game = Game::new(first_turn);
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut input = String::new();

    let concluded_game = 'done: loop {
        println!("\n{}\n", game.board);

        game = 'turn: loop {
            let tile: TileId = loop {
                input.clear();
                print!("{}'s turn: ", game.whos_turn());
                stdout.flush().expect("this should not fail");
                stdin.read_line(&mut input).expect("stdio read fucked");
                if let Ok(tile) = TileId::from_str(input.trim()) {
                    break tile;
                }
                println!("Invalid input! Try again.");
            };

            game = match game.mark(tile) {
                TurnResult::Retry(g) => {
                    println!("Invalid tile! Tile already marked. Try again.");
                    g
                }
                TurnResult::NextTurn(g) => break 'turn g,
                TurnResult::Concluded(conclusion) => break 'done conclusion,
            };
        }
    };

    match concluded_game.conclusion() {
        Conclusion::Win(player) => println!("{player} won!"),
        Conclusion::Draw => println!("Draw."),
    };
    println!("\n{}\n", concluded_game.board);
}

#[derive(Debug, PartialEq, Clone, Copy)]
enum Conclusion {
    Win(Player),
    Draw,
}

#[derive(Debug)]
pub struct InProgress {
    turn: Player,
}

#[derive(Debug)]
pub struct NewGame;

pub trait GameState {}
impl GameState for Conclusion {}
impl GameState for InProgress {}
impl GameState for NewGame {}

#[derive(Debug)]
pub struct Game<S: GameState = NewGame> {
    board: Board,
    state: S,
}

impl Game<NewGame> {
    pub fn new(first_turn: Player) -> Game<InProgress> {
        Game {
            board: Board::default(),
            state: InProgress { turn: first_turn },
        }
    }
}

enum TurnResult {
    Retry(Game<InProgress>),
    NextTurn(Game<InProgress>),
    Concluded(Game<Conclusion>),
}

impl Game<InProgress> {
    fn mark(mut self, tile: TileId) -> TurnResult {
        if !self.is_valid_mark(tile) {
            return TurnResult::Retry(self);
        }
        self.board.mark(tile, self.state.turn);

        if let Some(conclusion) = self.has_game_concluded() {
            TurnResult::Concluded(Game {
                state: conclusion,
                board: self.board,
            })
        } else {
            TurnResult::NextTurn(self.next_turn())
        }
    }

    fn whos_turn(&self) -> Player {
        self.state.turn
    }

    fn has_game_concluded(&self) -> Option<Conclusion> {
        let mark_count = self.board.mark_count();
        if mark_count < 3 {
            return None;
        }

        let a1 = self.board[TileId::A1];
        let a2 = self.board[TileId::A2];
        let a3 = self.board[TileId::A3];
        let b1 = self.board[TileId::B1];
        let b2 = self.board[TileId::B2];
        let b3 = self.board[TileId::B3];
        let c1 = self.board[TileId::C1];
        let c2 = self.board[TileId::C2];
        let c3 = self.board[TileId::C3];

        if let Some(player) = a1 {
            if (a1 == a3 && a1 == a2) || (a1 == c1 && a1 == b1) || (a1 == c3 && a1 == b2) {
                return Some(Conclusion::Win(player));
            }
        }

        if let Some(player) = c1 {
            if (c1 == c3 && c1 == c2) || (c1 == a3 && c1 == b2) {
                return Some(Conclusion::Win(player));
            }
        }

        if let Some(player) = b2 {
            if (b2 == a2 && b2 == c2) || (b2 == b1 && b2 == b3) {
                return Some(Conclusion::Win(player));
            }
        }

        if let Some(player) = a3 {
            if a3 == c3 && c3 == b3 {
                return Some(Conclusion::Win(player));
            }
        }

        if mark_count == 9 {
            return Some(Conclusion::Draw);
        }

        None
    }

    fn is_valid_mark(&self, tile: TileId) -> bool {
        self.board[tile].is_none()
    }

    fn next_turn(mut self) -> Self {
        self.state.turn = !self.state.turn;
        self
    }
}

impl Game<Conclusion> {
    pub fn conclusion(&self) -> Conclusion {
        self.state
    }
}
