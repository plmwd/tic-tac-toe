use std::{
    fmt::Display,
    ops::{Index, IndexMut, Not},
    str::FromStr,
};

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
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

// rank:
// 3: 6 7 8
// 2: 3 4 5
// 1: 0 1 2
//    A B C : file
#[derive(Debug, Default, PartialEq, PartialOrd, Clone, Copy)]
pub struct TileId(u8);
impl TileId {
    pub const A1: TileId = TileId(0);
    pub const B1: TileId = TileId(1);
    pub const C1: TileId = TileId(2);
    pub const A2: TileId = TileId(3);
    pub const B2: TileId = TileId(4);
    pub const C2: TileId = TileId(5);
    pub const A3: TileId = TileId(6);
    pub const B3: TileId = TileId(7);
    pub const C3: TileId = TileId(8);
}

impl FromStr for TileId {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "a1" | "A1" => Ok(TileId::A1),
            "a2" | "A2" => Ok(TileId::A2),
            "a3" | "A3" => Ok(TileId::A3),
            "b1" | "B1" => Ok(TileId::B1),
            "b2" | "B2" => Ok(TileId::B2),
            "b3" | "B3" => Ok(TileId::B3),
            "c1" | "C1" => Ok(TileId::C1),
            "c2" | "C2" => Ok(TileId::C2),
            "c3" | "C3" => Ok(TileId::C3),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Default, PartialEq, Clone, Serialize, Deserialize)]
pub struct Board {
    tiles: [Option<Player>; 9],
}

impl Board {
    pub fn mark(&mut self, tile: TileId, player: Player) {
        self[tile] = Some(player);
    }

    pub fn mark_count(&self) -> u8 {
        self.tiles.iter().flatten().count() as u8
    }
}

impl Index<TileId> for Board {
    type Output = Option<Player>;

    fn index(&self, tile: TileId) -> &Self::Output {
        &self.tiles[tile.0 as usize]
    }
}

impl IndexMut<TileId> for Board {
    fn index_mut(&mut self, index: TileId) -> &mut Self::Output {
        &mut self.tiles[index.0 as usize]
    }
}

impl Display for Board {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, rank) in self.tiles.chunks_exact(3).enumerate().rev() {
            write!(f, "{}│ ", i + 1)?;
            for tile in rank {
                match tile {
                    Some(player) => write!(f, "{player}")?,
                    None => write!(f, "-")?,
                };
            }

            writeln!(f)?;
        }
        write!(f, " ╰─────\n   ABC")?;
        Ok(())
    }
}
