pub use crate::board::{Board, Player, TileId};

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Conclusion {
    Win(Player),
    Draw,
}

#[derive(Debug)]
pub enum State {
    Playing(Player),
    Concluded(Conclusion),
}

#[derive(Debug)]
pub struct Game {
    pub board: Board,
    pub state: State,
}

impl Game {
    pub fn new(first_turn: Player) -> Self {
        Game {
            board: Board::default(),
            state: State::Playing(first_turn),
        }
    }

    pub fn try_mark_tile(&mut self, tile: TileId) -> bool {
        if !self.is_valid_mark(tile) {
            return false;
        }

        match self.state {
            State::Concluded(_) => false,
            State::Playing(turn) => {
                self.board.mark(tile, turn);
                true
            }
        }
    }

    pub fn has_game_concluded(&self) -> Option<Conclusion> {
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

    pub fn next_turn(&mut self) {
        if let State::Playing(player) = self.state {
            if let Some(conclusion) = self.has_game_concluded() {
                self.state = State::Concluded(conclusion);
            } else {
                self.state = State::Playing(!player)
            };
        }
    }
}
