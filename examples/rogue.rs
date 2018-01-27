extern crate curses_game_wrapper as cgw;
use cgw::{ActionResult, AsciiChar, GameSetting, Reactor};
use std::time::Duration;
fn main() {
    struct EmptyAI {
        loopnum: usize,
    };
    impl Reactor for EmptyAI {
        fn action(&mut self, _screen: ActionResult, turn: usize) -> Option<Vec<u8>> {
            let mut res = Vec::new();
            match turn {
                val if val == self.loopnum - 1 => res.push(AsciiChar::CarriageReturn.as_byte()),
                val if val == self.loopnum - 2 => res.push(b'y'),
                val if val == self.loopnum - 3 => res.push(b'Q'),
                _ => {
                    let c = match (turn % 4) as u8 {
                        0u8 => b'h',
                        1u8 => b'j',
                        2u8 => b'k',
                        _ => b'l',
                    };
                    res.push(c);
                }
            };
            Some(res)
        }
    }
    let loopnum = 50;
    let gs = GameSetting::new("rogue")
        .env("ROGUEUSER", "EmptyAI")
        .lines(24)
        .columns(80)
        .debug_file("debug.txt")
        .max_loop(loopnum + 1)
        .draw_on(Duration::from_millis(100));
    let game = gs.build();
    let mut ai = EmptyAI { loopnum: loopnum };
    game.play(&mut ai);
}
