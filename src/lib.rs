extern crate futures;
extern crate tokio_core;
extern crate tokio_process;
extern crate tokio_io;

use std::io::{self, Write};
use std::process::{Command, Stdio, ExitStatus};
use futures::{BoxFuture, Future, Stream};
use tokio_core::reactor::Core;
use tokio_process::{CommandExt, Child};
use std::fs::File;
use std::path::Path;
fn game_loop(mut cmd: Child) -> BoxFuture<ExitStatus, io::Error> {
    let stdin = cmd.stdin().take().unwrap();
    let stdout = cmd.stdout().take().unwrap();
    let mut writer = io::BufWriter::new(stdin);
    let reader = io::BufReader::new(stdout);
    // const BUFSIZE: usize = 32;
    // let mut readbuf = [0u8; BUFSIZE];
    // let read = tokio_io::io::read(reader, &mut readbuf);
    // let cycle = read.for_each(|r| {
    //     println!("Hello: {}", r);
    //     Ok(())
    // });
    let lines = tokio_io::io::lines(reader);
    let cycle = lines.for_each(move |l| {
        println!("{}", l);
        write!(writer, "q").unwrap();
        Ok(())
    });
    cycle.join(cmd).map(|((), s)| s).boxed()
}





fn spawn_game(ai_name: &str) {
    let mut core = Core::new().unwrap();
    let mut cmd = Command::new("rogue");
    // let mut cmd = cmd.env("USER", ai_name);
    let mut cmd = cmd.stdout(Stdio::piped()).stdin(Stdio::piped());
    let child = cmd.spawn_async(&core.handle()).unwrap();
    core.run(game_loop(child)).unwrap();
}



#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        ::spawn_game("hoge");
    }
}
