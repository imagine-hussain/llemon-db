#![allow(dead_code)]

use std::{
    error::Error,
    io::{self, Write},
    process::Command,
};

pub mod breakpoint;
pub mod prelude;
pub mod ptrace;

use prelude::*;

fn main() -> Result<(), Box<dyn Error>> {
    let mut input = String::new();

    let child_pid = launch_traceable(Command::new("./hello")).unwrap();
    println!("Attaching to program with pid {}", child_pid.0);

    let mut debugger = Debugger::from_pid(child_pid);

    loop {
        // this is macro cal, not a funciton
        print!(">>> ");
        input.clear();
        io::stdout().flush()?;
        io::stdin()
            .read_line(&mut input)
            .expect("Failed to read line");

        let line = input.trim();
        let mut inp = line.split_whitespace();
        let command = match inp.next() {
            Some(c) => c,
            None => continue,
        };

        match command {
            "run" => debugger.continue_process().unwrap(),
            "break" => {
                let addr_raw = inp.next().expect("Give address");
                let addr = isize::from_str_radix(addr_raw, 16).unwrap();

                debugger.add_breakpoint_at(addr).unwrap();
            }
            "exit" => unsafe {
                libc::kill(child_pid.0, libc::SIGKILL);
                return Ok(());
            },
            _ => {
                println!("Dont know command: {command}");
            }
        }

        // io::Result<>
        // foo::Error
        // foo::Result<T> = Result<T, foo::Error>
    }
}
