#![allow(dead_code)]

use std::{
    any::type_name, error::Error, io::{self, Write}, process::Command, str::FromStr
};

pub mod breakpoint;
pub mod dwarf;
pub mod prelude;
pub mod ptrace;
pub mod registers;
pub mod target;

use prelude::*;
use ptrace::peekdata;
use registers::dump_user_regs;

fn main() -> Result<(), Box<dyn Error>> {
    let mut input = String::new();

    let mut target = launch_traceable(Command::new("./hello")).unwrap();
    let child_pid = target.pid();
    println!("Attaching to program with pid {}", child_pid.0);

    loop {
        print!(">>> ");
        input.clear();
        io::stdout().flush()?;
        io::stdin()
            .read_line(&mut input)
            .expect("Failed to read line");

        let line: &mut str = &mut input;
        let mut inp = line.split_whitespace();
        let command = match inp.next() {
            Some(c) => c,
            None => continue,
        };

        match command {
            "continue" | "c" => target.continue_process().unwrap(),
            "break" => {
                let addr_raw = inp.next().expect("Give address");
                let addr = isize::from_str_radix(addr_raw, 16).unwrap();

                target.add_breakpoint_at(addr).unwrap();
            }
            "exit" => unsafe {
                libc::kill(child_pid.0, libc::SIGKILL);
                return Ok(());
            },
            "register" | "reg" => match inp.next() {
                Some("get" | "read" | "r") => {
                    let reg =
                        registers::Register::from_str(inp.next().unwrap().to_uppercase().as_str())?;

                    let value = ptrace::get_reg(child_pid, reg)?;
                    println!("Register has value: {value:x} = {value}");
                }
                Some("set" | "write" | "w") => {
                    let reg =
                        registers::Register::from_str(inp.next().unwrap().to_uppercase().as_str())?;

                    let value_str = inp.next().unwrap();
                    let value: u64 = value_str.parse()?;
                    ptrace::set_reg(child_pid, reg, value)?;
                }
                None => {
                    let regs = ptrace::get_regs(child_pid)?;
                    dump_user_regs(&regs);
                }
                _ => todo!("invalid input"),
            },
            "read" => {
                // read <addr>(:<type>)?
                let addr_and_type = inp.next().expect("Give address, optionally give a type");
                let (addr_str, typename) = match addr_and_type.split_once(':') {
                    Some((addr, ty)) => (addr, ty),
                    None => (addr_and_type, "i64"),
                };
                let addr: isize = addr_str.parse()?;

                peektype_and_print!(
                    typename, child_pid, addr,
                    i32, u32, i64, u64, char, bool, u8, i8, usize, isize, i16, u16,
                    f32, f64, i128, u128
                );
            },
            "write" => {
                // write <addr>(:type)? <value>
                let addr_and_type = inp.next().expect("Give address, optionally give a type");
                let (addr_str, typename) = match addr_and_type.split_once(':') {
                    Some((addr, ty)) => (addr, ty),
                    None => (addr_and_type, "i64"),
                };
                let addr: isize = addr_str.parse()?;
                let value_str = inp.next().unwrap();

                parsetype_and_poke!(
                    value_str, typename, child_pid, addr,
                    i32, u32, i64, u64, char, bool, u8, i8, usize, isize, i16, u16,
                    f32, f64, i128, u128
                );
            }
            _ => {
                println!("Dont know command: {command}");
            }
        }
    }
}
