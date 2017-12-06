extern crate futures;
extern crate tokio_core;
extern crate tokio_process;
#[macro_use]
extern crate tokio_io;

use std::process::{Command, Stdio, ExitStatus};
use futures::{BoxFuture, Future, Poll, Stream};
use tokio_core::reactor::Core;
use tokio_process::{CommandExt, Child};
use tokio_io::AsyncRead;
use std::io::{self, Write, BufRead};
use std::mem;

struct GameSetting {
    lines: usize,
    colomns: usize,
    username: String,
}

struct GameData {
    lines: usize,
    columns: usize,
}

struct GameRead<A> {
    io: A,
    buf: Vec<u8>,
}

impl<A> GameRead<A>
where
    A: AsyncRead + BufRead,
{
    fn new(a: A) -> GameRead<A> {
        GameRead {
            io: a,
            buf: vec![0; 1],
        }
    }
}

impl<A> Stream for GameRead<A>
where
    A: AsyncRead + BufRead,
{
    type Item = Vec<u8>;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Vec<u8>>, io::Error> {
        match try_nb!(self.io.read(&mut self.buf)) {
            val if val > 0 => {
                println!("read 1 bytes");
                Ok(Some(mem::replace(&mut self.buf, vec![0; 1])).into())
            }
            _ => {
                println!("read 1 bytes");
                Err(io::Error::new(io::ErrorKind::BrokenPipe, "hi"))
            }
        }
        // if self.buf[0] == 0 {
        //     Err(io::Error::new(io::ErrorKind::BrokenPipe, "hi"))
        // } else {
        //     Ok(Some(mem::replace(&mut self.buf, vec![0; 1])).into())
        // }
    }
}


fn game_loop(mut cmd: Child) -> BoxFuture<ExitStatus, io::Error> {
    // let stdin = cmd.stdin().take().unwrap();
    // let mut writer = io::BufWriter::new(stdin);
    let stdout = cmd.stdout().take().unwrap();
    let reader = io::BufReader::new(stdout);
    // const BUFSIZE: usize = 32;
    // let mut readbuf = [0u8; BUFSIZE];
    // let read = tokio_io::io::read(reader, &mut readbuf);
    // let cycle = read.for_each(|r| {
    //     println!("Hello: {}", r);
    //     Ok(())
    // });
    let read = GameRead::new(reader);
    let cycle = read.for_each(move |r| {
        println!("{:?}", r);
        // write!(writer, "q").unwrap();
        Ok(())
    });
    // let lines = tokio_io::io::lines(reader);
    // let cycle = lines.for_each(move |l| {
    //     println!("{}", l);
    //     write!(writer, "q").unwrap();
    //     Ok(())
    // });
    cycle.join(cmd).map(|((), s)| s).boxed()
}





fn spawn_game(ai_name: &str) {
    let mut core = Core::new().unwrap();
    let mut cmd = Command::new("rogue");
    let cmd = cmd.env("ROGUEUSER", ai_name).env("LINES", "1");
    let cmd = cmd.stdout(Stdio::piped()); //.stdin(Stdio::piped());
    let child = cmd.spawn_async(&core.handle()).unwrap();
    match core.run(game_loop(child)) {
        Ok(_) => {}
        Err(_) => println!("error"),
    }
}


#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        ::spawn_game("my-ai");
    }
}
