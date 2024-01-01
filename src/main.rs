use std::{
    fmt::Display,
    ops::{Index, IndexMut, Not},
};

const NUM_TILES: u8 = 9;

#[derive(Debug, Default, PartialEq, Clone, Copy)]
enum Player {
    #[default]
    O,
    X,
}

impl Not for Player {
    type Output = Self;

    fn not(self) -> Self::Output {
        match self {
            Player::O => Player::X,
            Player::X => Player::O,
        }
    }
}

// 2: 6 7 8
// 1: 3 4 5
// 0: 0 1 2
//    A B C
#[derive(Debug, Default, PartialEq, PartialOrd, Clone, Copy)]
struct TileId(u8);
impl TileId {
    const A0: TileId = TileId(0);
    const B0: TileId = TileId(1);
    const C0: TileId = TileId(2);
    const A1: TileId = TileId(3);
    const B1: TileId = TileId(4);
    const C1: TileId = TileId(5);
    const A2: TileId = TileId(6);
    const B2: TileId = TileId(7);
    const C2: TileId = TileId(8);

    const fn new(val: u8) -> Option<Self> {
        if val >= NUM_TILES {
            None
        } else {
            Some(Self(val))
        }
    }

    const fn xy(x: u8, y: u8) -> Option<Self> {
        if x >= 3 || y >= 3 {
            None
        } else {
            Some(Self(y * 3 + x))
        }
    }
}

#[derive(Debug, Default, PartialEq, Clone)]
struct Board {
    tiles: [Option<Player>; 9],
}

impl Board {
    fn mark(&mut self, index: TileId, player: Player) {
        self[index] = Some(player);
    }

    fn mark_count(&self) -> u8 {
        self.tiles.iter().flatten().count() as u8
    }
}

impl Index<TileId> for Board {
    type Output = Option<Player>;

    fn index(&self, index: TileId) -> &Self::Output {
        &self.tiles[index.0 as usize]
    }
}

impl IndexMut<TileId> for Board {
    fn index_mut(&mut self, index: TileId) -> &mut Self::Output {
        &mut self.tiles[index.0 as usize]
    }
}

impl Display for Board {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, tile) in self.tiles.iter().rev().enumerate() {
            match tile {
                Some(Player::O) => write!(f, "O")?,
                Some(Player::X) => write!(f, "X")?,
                None => write!(f, "-")?,
            };

            if i % 3 == 2 {
                writeln!(f)?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
enum Conclusion {
    Win(Player),
    Draw,
}

#[derive(Debug)]
struct InProgress {
    turn: Player,
}

struct NewGame;

trait GameState {}
impl GameState for Conclusion {}
impl GameState for InProgress {}
impl GameState for NewGame {}

#[derive(Debug)]
struct Game<S: GameState = NewGame> {
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

// TODO: retry reason?
enum TurnResult {
    Retry(Game<InProgress>),
    NextTurn(Game<InProgress>),
    Concluded(Game<Conclusion>),
}

impl Game<InProgress> {
    pub fn mark(mut self, tile: TileId) -> TurnResult {
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

    fn has_game_concluded(&self) -> Option<Conclusion> {
        let mark_count = self.board.mark_count();
        if mark_count <= 3 {
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

fn main() {
    let mut game = Game::new(Player::X);
    println!("{}", game.board);
    game.board.mark(TileId::B1, Player::O);
    println!("{}", game.board);
}
