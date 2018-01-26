//! This crate is wrapper of curses games like rogue and nethack, for AI making.
//!
//! What this crate provie is spawning CUI game as child process and emulation of vt100 control
//! sequence(helped by vte crate).
//!
//! To run AI, You have to implement ```Reactor``` trait to your AI object.
//! The result of vt100 emulation are stored as ```Vec<Vec<u8>>``` and AI recieves it as
//! ```Changed(Vec<Vec<u8>>)```. # Examples
//! ```
//! extern crate curses_game_wrapper as cgw;
//! use cgw::{Reactor, ActionResult, AsciiChar, GameSetting, Severity};
//! use std::time::Duration;
//! fn main() {
//!     struct EmptyAI {
//!         loopnum: usize,
//!     };
//!     impl Reactor for EmptyAI {
//!         fn action(&mut self, _screen: ActionResult, turn: usize) -> Option<Vec<u8>> {
//!             let mut res = Vec::new();
//!             match turn {
//!                 val if val == self.loopnum - 1 => res.push(AsciiChar::CarriageReturn.as_byte()),
//!                 val if val == self.loopnum - 2 => res.push(b'y'),
//!                 val if val == self.loopnum - 3 => res.push(b'Q'),
//!                 _ => {
//!                     let c = match (turn % 4) as u8 {
//!                         0u8 => b'h',
//!                         1u8 => b'j',
//!                         2u8 => b'k',
//!                         _ => b'l',
//!                     };
//!                     res.push(c);
//!                 }
//!             };
//!             Some(res)
//!         }
//!     }
//!     let loopnum = 50;
//!     let gs = GameSetting::new("rogue")
//!         .env("ROGUEUSER", "EmptyAI")
//!         .lines(24)
//!         .columns(80)
//!         .debug_file("debug.txt")
//!         .max_loop(loopnum + 1)
//!         .draw_on(Duration::from_millis(200));
//!     let game = gs.build();
//!     let mut ai = EmptyAI { loopnum: loopnum };
//!     game.play(&mut ai);
//! }
//! ```

#![cfg_attr(feature = "clippy", feature(plugin))]
#![cfg_attr(feature = "clippy", plugin(clippy))]

extern crate ascii;
#[macro_use]
extern crate bitflags;
#[macro_use]
extern crate slog;
extern crate sloggers;
extern crate vte;

mod term_data;

/// It's imported from ```ascii``` crate for convinience.
pub use ascii::AsciiChar;
pub use sloggers::types::Severity;
use std::error::Error;
use std::fmt::{self, Debug, Formatter};
use std::io;
use std::io::{BufReader, Read, Write};
use std::process::{Child, Command, Stdio};
use std::str;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use term_data::TermData;
use vte::Parser;

#[derive(Clone, Debug)]
struct LogInfo {
    fname: String,
    sev: Severity,
}

impl Default for LogInfo {
    fn default() -> LogInfo {
        LogInfo {
            fname: String::new(),
            sev: Severity::Debug,
        }
    }
}

#[derive(Copy, Clone, Debug)]
enum DrawType {
    Terminal(Duration),
    Null,
}

/// Game process builder, providing control over how a new process
/// should be spawned.
///
/// Like ```std::process::Command```, A default configuration can be
/// generated using Gamesetting::new(command name) and other settings
/// can be added by builder methods.
/// # Example
/// ```
/// extern crate curses_game_wrapper as cgw;
/// use cgw::{Reactor, ActionResult, AsciiChar, GameSetting, Severity};
/// use std::time::Duration;
/// fn main() {
///     let loopnum = 50;
///     let gs = GameSetting::new("rogue")
///         .env("ROGUEUSER", "EmptyAI")
///         .lines(24)
///         .columns(80)
///         .debug_file("debug.txt")
///         .max_loop(loopnum + 1)
///         .draw_on(Duration::from_millis(200));
///     let game = gs.build();
/// }
/// ```
#[derive(Clone, Debug)]
pub struct GameSetting<'a> {
    cmdname: String,
    lines: usize,
    columns: usize,
    envs: Vec<(&'a str, &'a str)>,
    args: Vec<&'a str>,
    log_info: LogInfo,
    timeout: Duration,
    draw_type: DrawType,
    max_loop: usize,
}
impl<'a> GameSetting<'a> {
    /// Build GameSetting object with command name(like ```rogue```).
    pub fn new(command_name: &str) -> Self {
        GameSetting {
            cmdname: String::from(command_name),
            lines: 24,
            columns: 80,
            envs: Vec::new(),
            args: Vec::new(),
            log_info: LogInfo::default(),
            timeout: Duration::from_millis(100),
            draw_type: DrawType::Null,
            max_loop: 100,
        }
    }
    /// Set screen width of curses widow
    pub fn columns(mut self, u: usize) -> Self {
        self.columns = u;
        self
    }
    /// Set screen height of curses window
    pub fn lines(mut self, u: usize) -> Self {
        self.lines = u;
        self
    }
    /// Add command line argument
    pub fn arg(mut self, s: &'a str) -> Self {
        self.args.push(s);
        self
    }
    /// Set environmental variable
    pub fn env(mut self, s: &'a str, t: &'a str) -> Self {
        self.envs.push((s, t));
        self
    }
    /// Set multiple command line arguments
    pub fn args<I>(mut self, i: I) -> Self
    where
        I: IntoIterator<Item = &'a str>,
    {
        let v: Vec<_> = i.into_iter().map(|x| x).collect();
        self.args = v;
        self
    }
    /// Set multiple environmental variables
    pub fn envs<I>(mut self, i: I) -> Self
    where
        I: IntoIterator<Item = (&'a str, &'a str)>,
    {
        let v: Vec<_> = i.into_iter().map(|(s, t)| (s, t)).collect();
        self.envs = v;
        self
    }
    /// Draw game on terminal(Default: off).
    /// You hanve to set duration of drawing.
    pub fn draw_on(mut self, d: Duration) -> Self {
        self.draw_type = DrawType::Terminal(d);
        self
    }
    /// You can set debug file of this crate.
    /// This is mainly for developper of this crate:)
    pub fn debug_file(mut self, s: &str) -> Self {
        self.log_info.fname = s.to_owned();
        self
    }
    /// You can set debug level of this crate.
    /// This is mainly for developper of this crate:)
    pub fn debug_level(mut self, s: Severity) -> Self {
        self.log_info.sev = s;
        self
    }
    /// You can set timeout to game output.
    /// It's setted to 0.1s by default.
    pub fn timeout(mut self, d: Duration) -> Self {
        self.timeout = d;
        self
    }
    /// You can set max_loop of game.
    /// It's setted to 100 by default.
    pub fn max_loop(mut self, t: usize) -> Self {
        self.max_loop = t;
        self
    }
    /// Consume game setting and build GameEnv
    pub fn build(self) -> GameEnv {
        let dat = TermData::from_setting(&self);
        let t = self.timeout;
        let m = self.max_loop;
        let d = self.draw_type;
        GameEnv {
            process: ProcHandler::from_setting(self),
            term_data: dat,
            timeout: t,
            max_loop: m,
            draw_type: d,
        }
    }
}

/// Result of the game action.
/// ```Changed(Vec<Vec<u8>>)``` contains virtual terminal as buffer.
#[derive(Clone)]
pub enum ActionResult {
    Changed(Vec<Vec<u8>>),
    NotChanged,
    GameEnded,
}
impl Debug for ActionResult {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        match *self {
            ActionResult::Changed(ref buf) => {
                write!(f, "ActionResult::Changed\n")?;
                write!(f, "--------------------\n")?;
                for v in buf {
                    let s = str::from_utf8(v).unwrap();
                    write!(f, "{}\n", s)?;
                }
                write!(f, "--------------------")
            }
            ActionResult::NotChanged => write!(f, "ActionResult::NotChanged"),
            ActionResult::GameEnded => write!(f, "ActionResult::GameEnded"),
        }
    }
}

/// You have to implement ```Reactor``` for your AI to work.
pub trait Reactor {
    fn action(&mut self, action_result: ActionResult, turn: usize) -> Option<Vec<u8>>;
}

/// This is for spawning curses game as child process.
///
/// It stores inputs from the game and sends result to AI when its input handler timeouts.
///
/// The only usage is
/// # Example
/// ```
/// extern crate curses_game_wrapper as cgw;
/// use cgw::{Reactor, ActionResult, AsciiChar, GameSetting, Severity};
/// use std::time::Duration;
/// fn main() {
///     struct EmptyAI;
///     impl Reactor for EmptyAI {
///         fn action(&mut self, _screen: ActionResult, _turn: usize) -> Option<Vec<u8>> {
///              None
///         }
///     }
///     let gs = GameSetting::new("rogue")
///         .env("ROGUEUSER", "EmptyAI")
///         .lines(24)
///         .columns(80)
///         .debug_file("debug.txt")
///         .max_loop(10)
///         .draw_on(Duration::from_millis(200));
///     let game = gs.build();
///     let mut ai = EmptyAI { };
///     game.play(&mut ai);
/// }
/// ```
pub struct GameEnv {
    process: ProcHandler,
    term_data: TermData,
    timeout: Duration,
    max_loop: usize,
    draw_type: DrawType,
}
impl GameEnv {
    /// Start process and run AI.
    /// 
    pub fn play<R: Reactor>(mut self, ai: &mut R) {
        use mpsc::RecvTimeoutError;
        macro_rules! send_or {
            ($to:expr, $handle:expr) => (
                if let Err(why) = $to.send_bytes($handle) {
                    debug!(
                        self.term_data.logger,
                        concat!("can't send to ", stringify!($to), ": {}"),
                        why.description()
                    );
                }
            )
        }
        let proc_handle = self.process.run();
        let mut viewer: Box<GameViewer> = match self.draw_type {
            DrawType::Terminal(d) => Box::new(TerminalViewer::new(d)),
            DrawType::Null => Box::new(EmptyViewer {}),
        };
        let viewer_handle = viewer.run();
        let mut parser = Parser::new();
        let mut proc_dead = false;
        let mut stored_map = None;
        for i in 0..self.max_loop {
            macro_rules! do_action {
                ($act:expr) => {
                    if let Some(bytes) = ai.action($act, i) {
                        send_or!(self.process, &bytes);
                    }
                }
            }
            if proc_dead {
                trace!(self.term_data.logger, "Game ended in turn {}", i - 1);
                break;
            }
            let action_res = match self.process.rx.recv_timeout(self.timeout) {
                Ok(rec) => match rec {
                    Handle::Panicked => {
                        send_or!(viewer, Handle::Panicked);
                        panic!("panicked in child thread")
                    }
                    Handle::Zero => {
                        debug!(self.term_data.logger, "read zero bytes");
                        send_or!(viewer, Handle::Zero);
                        proc_dead = true;
                        ActionResult::GameEnded
                    }
                    Handle::Valid(ref r) => {
                        send_or!(viewer, Handle::Valid(r));
                        for c in r {
                            parser.advance(&mut self.term_data, *c);
                        }
                        ActionResult::Changed(self.term_data.ret_screen())
                    }
                },
                Err(err) => match err {
                    RecvTimeoutError::Timeout => ActionResult::NotChanged,
                    RecvTimeoutError::Disconnected => panic!("disconnected"),
                },
            };
            trace!(self.term_data.logger, "{:?}, turn: {}", action_res, i);
            match action_res {
                ActionResult::GameEnded => do_action!(ActionResult::GameEnded),
                // store inputs until timeout occurs
                ActionResult::Changed(map) => stored_map = Some(map),
                ActionResult::NotChanged => if let Some(map) = stored_map {
                    do_action!(ActionResult::Changed(map));
                    stored_map = None;
                } else {
                    do_action!(ActionResult::NotChanged);
                },
            }
        }
        if !proc_dead {
            debug!(
                self.term_data.logger,
                "Game not ended and killed process forcibly"
            );
            self.process.kill();
            send_or!(viewer, Handle::Zero);
            let _ = ai.action(ActionResult::GameEnded, self.max_loop);
        }
        proc_handle.join().unwrap();
        viewer_handle.join().unwrap();
    }
}

// handles Sender and Reciever
enum Handle<T> {
    Panicked, // thread panicked
    Zero,     // read 0 bytes (probably game ended)
    Valid(T), // read 1 or more bytes
}

trait GameViewer {
    fn run(&mut self) -> JoinHandle<()>;
    fn send_bytes(&mut self, bytes: Handle<&[u8]>) -> Result<(), ViewerError>;
}

#[derive(Debug)]
struct ViewerError(String);
impl fmt::Display for ViewerError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl Error for ViewerError {
    fn description(&self) -> &str {
        &self.0
    }
}
impl From<mpsc::SendError<Handle<Vec<u8>>>> for ViewerError {
    fn from(e: mpsc::SendError<Handle<Vec<u8>>>) -> Self {
        ViewerError(e.description().to_owned())
    }
}

struct EmptyViewer {}

impl GameViewer for EmptyViewer {
    fn run(&mut self) -> JoinHandle<()> {
        thread::spawn(move || {})
    }
    fn send_bytes(&mut self, _bytes: Handle<&[u8]>) -> Result<(), ViewerError> {
        Ok(())
    }
}

#[derive(Debug)]
struct TerminalViewer {
    tx: mpsc::Sender<Handle<Vec<u8>>>,
    rx: Arc<Mutex<Receiver<Handle<Vec<u8>>>>>,
    sleep_time: Arc<Duration>,
}

impl TerminalViewer {
    fn new(d: Duration) -> Self {
        let (tx, rx) = mpsc::channel();
        let wrapped_recv = Arc::new(Mutex::new(rx));
        TerminalViewer {
            tx: tx,
            rx: wrapped_recv,
            sleep_time: Arc::new(d),
        }
    }
}
impl GameViewer for TerminalViewer {
    fn run(&mut self) -> JoinHandle<()> {
        let rx = Arc::clone(&self.rx);
        let sleep = Arc::clone(&self.sleep_time);
        thread::spawn(move || {
            let receiver = rx.lock().unwrap();
            while let Ok(game_input) = (*receiver).recv() {
                match game_input {
                    Handle::Valid(ref bytes) => {
                        let s = str::from_utf8(bytes).unwrap();
                        print!("{}", s);
                        io::stdout().flush().expect("Could not flush stdout");
                    }
                    Handle::Zero => break,
                    Handle::Panicked => panic!("main thread panicked"),
                }
                thread::sleep(*sleep);
            }
        })
    }
    fn send_bytes(&mut self, b: Handle<&[u8]>) -> Result<(), ViewerError> {
        let txclone = self.tx.clone();
        let res = match b {
            Handle::Zero => Handle::Zero,
            Handle::Panicked => Handle::Panicked,
            Handle::Valid(b) => Handle::Valid(b.to_owned()),
        };
        txclone.send(res)?;
        Ok(())
    }
}

#[derive(Debug)]
struct ProcessError(String);

impl fmt::Display for ProcessError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Error for ProcessError {
    fn description(&self) -> &str {
        &self.0
    }
}

impl From<io::Error> for ProcessError {
    fn from(why: io::Error) -> Self {
        ProcessError(why.description().to_owned())
    }
}

// exec process
struct ProcHandler {
    my_proc: Child,
    tx: Sender<Handle<Vec<u8>>>,
    // note : Reciever blocks until some bytes wrote
    rx: Receiver<Handle<Vec<u8>>>,
    killed: Arc<AtomicBool>,
}

impl ProcHandler {
    fn from_setting(g: GameSetting) -> ProcHandler {
        let mut cmd = Command::new(&g.cmdname);
        let cmd = cmd.args(g.args);
        let cmd = cmd.env("LINES", format!("{}", g.lines));
        let cmd = cmd.env("COLUMNS", format!("{}", g.columns));
        let cmd = cmd.env("TERM", "vt100"); // You can override it by env
        let cmd = cmd.envs(g.envs);
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
            killed: Arc::new(AtomicBool::new(false)),
        }
    }

    fn run(&mut self) -> JoinHandle<()> {
        let proc_out = self.my_proc.stdout.take().unwrap();
        let txclone = self.tx.clone();
        let ac = Arc::clone(&self.killed);
        thread::spawn(move || {
            let mut proc_reader = BufReader::new(proc_out);
            const BUFSIZE: usize = 4096;
            let mut readbuf = vec![0u8; BUFSIZE];
            while !ac.load(Ordering::Relaxed) {
                match proc_reader.read(&mut readbuf) {
                    Err(why) => {
                        txclone.send(Handle::Panicked).ok();
                        panic!("couldn't read child stdout: {}", why.description())
                    }
                    Ok(0) => {
                        txclone.send(Handle::Zero).ok();
                        break;
                    }
                    Ok(BUFSIZE) => {
                        txclone.send(Handle::Panicked).ok();
                        panic!("Buffer is too small.")
                    }
                    Ok(n) => {
                        txclone.send(Handle::Valid(readbuf[0..n].to_owned())).ok();
                    }
                }
            }
        })
    }

    fn send_bytes(&mut self, buf: &[u8]) -> Result<(), ProcessError> {
        let stdin = self.my_proc.stdin.as_mut().unwrap();
        stdin.write_all(buf)?;
        Ok(())
    }

    fn kill(&mut self) {
        self.my_proc.kill().unwrap();
        let ac = Arc::clone(&self.killed);
        ac.store(true, Ordering::Relaxed)
    }
}

// Destractor (kill proc)
impl Drop for ProcHandler {
    fn drop(&mut self) {
        self.my_proc.kill().unwrap();
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        use ::*;
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
}
