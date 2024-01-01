use std::{
    fmt::Display,
    ops::{Index, IndexMut, Not},
    str::FromStr,
};

#[derive(Debug, Default, PartialEq, Clone, Copy)]
pub enum Player {
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

impl Display for Player {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Player::O => write!(f, "O"),
            Player::X => write!(f, "X"),
        }
    }
}

// 2: 6 7 8
// 1: 3 4 5
// 0: 0 1 2
//    A B C
#[derive(Debug, Default, PartialEq, PartialOrd, Clone, Copy)]
pub struct TileId(u8);
impl TileId {
    pub const A0: TileId = TileId(0);
    pub const B0: TileId = TileId(1);
    pub const C0: TileId = TileId(2);
    pub const A1: TileId = TileId(3);
    pub const B1: TileId = TileId(4);
    pub const C1: TileId = TileId(5);
    pub const A2: TileId = TileId(6);
    pub const B2: TileId = TileId(7);
    pub const C2: TileId = TileId(8);
}

impl FromStr for TileId {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "a0" | "A0" => Ok(TileId::A0),
            "a1" | "A1" => Ok(TileId::A1),
            "a2" | "A2" => Ok(TileId::A2),
            "b0" | "B0" => Ok(TileId::B0),
            "b1" | "B1" => Ok(TileId::B1),
            "b2" | "B2" => Ok(TileId::B2),
            "c0" | "C0" => Ok(TileId::C0),
            "c1" | "C1" => Ok(TileId::C1),
            "c2" | "C2" => Ok(TileId::C2),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct Board {
    tiles: [Option<Player>; 9],
}

impl Board {
    pub fn mark(&mut self, index: TileId, player: Player) {
        self[index] = Some(player);
    }

    pub fn mark_count(&self) -> u8 {
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
        for rank in self.tiles.chunks_exact(3).rev() {
            for tile in rank {
                match tile {
                    Some(player) => write!(f, "{player}")?,
                    None => write!(f, "-")?,
                };
            }
            writeln!(f)?;
        }
        Ok(())
    }
}
