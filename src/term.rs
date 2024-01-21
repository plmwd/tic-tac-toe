use crate::game::*;
use std::{
    io::{self, Write},
    str::FromStr,
};

pub fn play(first_turn: Player) {
    let mut game = Game::new(first_turn);
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut input = String::new();

    let mut prompt_for_tile = |turn| loop {
        input.clear();
        print!("{}'s turn: ", turn);
        stdout.flush().expect("this should not fail");
        stdin.read_line(&mut input).expect("stdio read fucked");
        if let Ok(tile) = TileId::from_str(input.trim()) {
            return tile;
        }
        println!("Invalid input! Try again.");
    };

    let conclusion = loop {
        match game.state {
            State::Concluded(conclusion) => break conclusion,
            State::Playing(whos_turn) => {
                println!("\n{}\n", game.board);

                loop {
                    let tile = prompt_for_tile(whos_turn);
                    if game.try_mark_tile(tile) {
                        break;
                    }
                    println!("Invalid tile! Tile already marked. Try again.");
                }
                game.next_turn();
            }
        };
    };

    match conclusion {
        Conclusion::Win(player) => println!("{player} won!"),
        Conclusion::Draw => println!("Draw."),
    };
    println!("\n{}\n", game.board);
}
