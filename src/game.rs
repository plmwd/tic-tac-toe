use crate::board::{Board, Player, TileId};
use std::{
    io::{self, Write},
    str::FromStr,
};

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

        let a0 = self.board[TileId::A0];
        let a1 = self.board[TileId::A1];
        let a2 = self.board[TileId::A2];
        let b0 = self.board[TileId::B0];
        let b1 = self.board[TileId::B1];
        let b2 = self.board[TileId::B2];
        let c0 = self.board[TileId::C0];
        let c1 = self.board[TileId::C1];
        let c2 = self.board[TileId::C2];

        if let Some(player) = a0 {
            if (a0 == a2 && a0 == a1) || (a0 == c0 && a0 == b0) || (a0 == c2 && a0 == b1) {
                return Some(Conclusion::Win(player));
            }
        }

        if let Some(player) = c0 {
            if (c0 == c2 && c0 == c1) || (c0 == a2 && c0 == b1) {
                return Some(Conclusion::Win(player));
            }
        }

        if let Some(player) = b1 {
            if (b1 == a1 && b1 == c1) || (b1 == b0 && b1 == b2) {
                return Some(Conclusion::Win(player));
            }
        }

        if let Some(player) = a2 {
            if a2 == c2 && c2 == b2 {
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

pub fn play(mut game: Game<InProgress>) {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut input = String::new();

    let conclusion = loop {
        println!("\n{}\n", game.board);

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
                println!("Invalid tile");
                g
            }
            TurnResult::NextTurn(g) => g,
            TurnResult::Concluded(conclusion) => break conclusion,
        };
    };

    match conclusion.conclusion() {
        Conclusion::Win(player) => println!("{player} won!"),
        Conclusion::Draw => println!("Draw."),
    };
    println!("\n{}\n", conclusion.board);
}
