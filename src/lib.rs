extern crate vte;
#[macro_use]
extern crate slog;
extern crate sloggers;
#[macro_use]
extern crate bitflags;
mod game_data;

use game_data::GameData;
use std::process::{Command, Stdio, Child};
use std::io::{Read, Write, BufRead, BufReader};
use std::sync::mpsc;
use std::thread;
use std::str;
use std::error::Error;
use std::ffi::{OsStr, OsString};
use std::time::Duration;
use vte::Parser;
pub use sloggers::types::Severity;
// sloggers::types::Severity
// pub enum Severity {
//     Trace,
//     Debug,
//     Info,
//     Warning,
//     Error,
//     Critical,
// }

#[derive(Debug)]
pub enum LogType {
    File((String, Severity)),
    Stdout(Severity),
    Stderr(Severity),
    None,
}
/// You can choose how game is drawn in screen.
/// If Realtime is chosen, game is drawn as AI is playing, and if
/// Restore chosen drawing starts when the game is over.
/// And you can also choose sleep time if Realtime or Restore is
/// chosen.
/// '''
/// '''
#[derive(Copy, Clone, Debug)]
pub enum GameShowType {
    RealTime(Duration),
    Restore(Duration),
    None,
}

/// Game process builder, providing control over how a new process
/// should be spawned.
/// Like std::process::Command struct, A default configuration can be
/// generated using Gamesetting::new(command name) and other settings
/// can be added by builder methods
pub struct GameSetting {
    lines: usize,
    columns: usize,
    cmdname: String,
    envs: Vec<(OsString, OsString)>,
    args: Vec<OsString>,
    game_show: GameShowType,
    debug_log: LogType,
    timeout: Duration,
}
impl GameSetting {
    pub fn new(s: &str) -> GameSetting {
        GameSetting {
            lines: 24,
            columns: 80,
            cmdname: String::from(s),
            envs: Vec::new(),
            args: Vec::new(),
            game_show: GameShowType::None,
            debug_log: LogType::None,
            timeout: Duration::from_millis(100),
        }
    }
    pub fn columns(mut self, u: usize) -> GameSetting {
        self.columns = u;
        self
    }
    pub fn lines(mut self, u: usize) -> GameSetting {
        self.lines = u;
        self
    }
    pub fn arg<S: AsRef<OsStr>>(mut self, s: S) -> GameSetting {
        self.args.push(s.as_ref().to_owned());
        self
    }
    pub fn env<S: AsRef<OsStr>>(mut self, s: S, t: S) -> GameSetting {
        self.envs.push(
            (s.as_ref().to_owned(), t.as_ref().to_owned()),
        );
        self
    }
    pub fn args<I, S>(mut self, i: I) -> GameSetting
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let v: Vec<OsString> = i.into_iter().map(|x| x.as_ref().to_owned()).collect();
        self.args = v;
        self
    }
    pub fn envs<I, K, V>(mut self, i: I) -> GameSetting
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        let v: Vec<(OsString, OsString)> =
            i.into_iter()
             .map(|(s, t)| (s.as_ref().to_owned(), t.as_ref().to_owned()))
             .collect();
        self.envs = v;
        self
    }
    pub fn show_type(mut self, t: GameShowType) -> GameSetting {
        self.game_show = t;
        self
    }
    pub fn debug_type(mut self, l: LogType) -> GameSetting {
        self.debug_log = l;
        self
    }
    pub fn timeout(mut self, d: Duration) -> GameSetting {
        self.timeout = d;
        self
    }
    pub fn build(self) -> GameEnv {
        let dat = GameData::from_setting(&self);
        let t = self.timeout;
        GameEnv {
            process: ProcHandler::from_setting(self),
            game_data: dat,
            timeout: t,
            parser: Parser::new(),
        }
    }
}

pub struct GameEnv {
    process: ProcHandler,
    game_data: GameData,
    timeout: Duration,
    parser: Parser,
}

// exec process
struct ProcHandler {
    my_proc: Child,
    tx: mpsc::Sender<Handle<Vec<u8>>>,
    // note : Reciever blocks until some bytes wrote
    rx: mpsc::Receiver<Handle<Vec<u8>>>,
}

// handles Sender and Reciever
enum Handle<T> {
    Panicked, // thread panicked
    Zero, // read 0 bytes (probably game ended)
    Valid(T), // read 1 or more bytes
}

impl ProcHandler {
    fn from_setting(g: GameSetting) -> ProcHandler {
        let mut cmd = Command::new(&g.cmdname);
        let cmd = cmd.args(g.args);
        let cmd = cmd.envs(g.envs);
        let cmd = cmd.env("LINES", format!("{}", g.lines));
        let cmd = cmd.env("COLUMNS", format!("{}", g.columns));
        let cmd = cmd.stdin(Stdio::piped()).stdout(Stdio::piped());
        let process = match cmd.spawn() {
            Ok(p) => p,
            Err(why) => panic!("couldn't spawn game: {}", why.description()),
        };
        let (tx, rx) = mpsc::channel();
        ProcHandler {
            my_proc: process,
            tx: tx,
            rx: rx,
        }
    }
    fn run(&mut self) {
        let proc_out = self.my_proc.stdout.take().unwrap();
        let txclone = self.tx.clone();
        thread::spawn(move || {
            let mut proc_reader = BufReader::new(proc_out);
            const BUFSIZE: usize = 2048;
            loop {
                let mut readbuf: [u8; BUFSIZE] = [0; BUFSIZE];
                match proc_reader.read(&mut readbuf) {
                    Err(why) => {
                        txclone.send(Handle::Panicked).ok();
                        panic!("couldn't read rogue stdout: {}", why.description())
                    }
                    Ok(BUFSIZE) => {
                        txclone.send(Handle::Panicked).ok();
                        panic!("Buffer is too small.")
                    }
                    Ok(0) => {
                        txclone.send(Handle::Zero).ok();
                        break;
                    }
                    Ok(n) => {
                        txclone.send(Handle::Valid(readbuf[0..n].to_owned())).ok();
                    }
                }
            }
        });
    }


    fn write(&mut self, buf: &[u8]) {
        let stdin = self.my_proc.stdin.as_mut().unwrap();
        match stdin.write_all(buf) {
            Err(why) => panic!("couldn't write to child's stdin: {}", why.description()),
            Ok(_) => {}
        }
    }
}

// Destractor (kill proc)
impl Drop for ProcHandler {
    fn drop(&mut self) {
        match self.my_proc.kill() {
            Ok(_) => println!("Killed Process"),
            Err(_) => println!("SIGKILL failed"),
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {}
}
