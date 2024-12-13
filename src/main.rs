#![allow(dead_code)]

use std::{
    error::Error,
    io::{self, Write},
    process::Command,
};

pub mod prelude;
pub mod ptrace;

use prelude::*;

fn main() -> Result<(), Box<dyn Error>> {
    let mut input = String::new();

    let child_pid = launch_traceable(Command::new("./hello")).unwrap();

    let mut debugger = Debugger::from_pid(child_pid);

    loop {
        // this is macro cal, not a funciton
        print!(">>> ");
        io::stdout().flush()?;
        io::stdin()
            .read_line(&mut input)
            .expect("Failed to read line");

        let line = input.trim();
        dbg!(line);
        if line == "run" {
            debugger.continue_process();
        }

        // io::Result<>
        // foo::Error
        // foo::Result<T> = Result<T, foo::Error>
    }
}
