#![allow(dead_code)]

use std::{
    error::Error,
    io::{self, Write},
    process::Command,
    str::FromStr,
};
use libc::isdigit;

pub mod breakpoint;
pub mod dwarf;
pub mod prelude;
pub mod ptrace;
pub mod registers;
pub mod target;

pub mod mmap;

use prelude::*;
use crate::dwarf::{DwarfInfo, StaticEndianSlice};

fn main() -> Result<(), Box<dyn Error>> {
    let mut input = String::new();

    let mut target = launch_traceable(Command::new("./hello")).unwrap();
    let child_pid = target.pid();
    println!("Attaching to program with pid {}", child_pid.0);

    let mut raw_dwarf = dwarf::read_dwarf("./hello").map_err(|e| dbg!(e))?;
    dwarf::process_dwarf_test::<StaticEndianSlice>(&mut raw_dwarf).map_err(|e| dbg!(e)).unwrap();
    let mut dwinfo = dwarf::DwarfInfo::new(raw_dwarf);

    loop {
        print!(">>> ");
        input.clear();
        io::stdout().flush()?;
        io::stdin()
            .read_line(&mut input)
            .expect("Failed to read line");

        if let Err(e) = run_command(&mut target, &mut dwinfo, &input) {
            println!("Error: {e}");
        };
    }
}

fn run_command(target: &mut target::Target, dwinfo: &mut DwarfInfo, line: &str) -> Result<(), Box<dyn Error>> {
    let mut inp = line.split_whitespace();
    let command = match inp.next() {
        Some(c) => c,
        None => return Ok(()),
    };
    let child_pid = target.pid();

    match command {
        "continue" | "c" => target.continue_process()?,
        "break" => {
            // break <address|function_name>
            let location = inp.next().expect("Give location to add the breakpoint");
            if location.starts_with("0x") {
                let addr = isize::from_str_radix(location, 16)?;
                target.add_breakpoint_at(addr)?;
            }
            else if location.chars().all(|c| c.is_ascii_digit()) {
                let addr = isize::from_str_radix(location, 10)?;
                target.add_breakpoint_at(addr)?;
            } else {
                let function_name = location;
                target.add_breakpoint_at_function(function_name)?;
                println!("Added breakpoint at function {function_name}");
            }
        }
        "exit" => {
            target.kill()?;
            std::process::exit(0);
        }
        "register" | "reg" => match inp.next() {
            Some("get" | "read" | "r") => {
                let register_name = inp.next().ok_or("Expecting register name to read")?;
                let reg =
                    registers::Register::from_str(register_name.to_uppercase().as_str())?;

                let value = ptrace::get_reg(child_pid, reg)?;
                println!("Register has value: {value:x} = {value}");
            }
            Some("set" | "write" | "w") => {
                let register_name = inp.next().ok_or("Invalid register name")?;
                let reg = registers::Register::from_str(register_name.to_uppercase().as_str())?;

                let value_str = inp.next().ok_or("Expect value to set register to")?;
                let value: u64 = value_str.parse()?;
                ptrace::set_reg(child_pid, reg, value)?;
            }
            None => {
                let regs = ptrace::get_regs(child_pid)?;
                registers::dump_user_regs(&regs);
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
                typename, child_pid, addr, i32, u32, i64, u64, char, bool, u8, i8, usize, isize,
                i16, u16, f32, f64, i128, u128
            );
        }
        "write" => {
            // write <addr>(:type)? <value>
            let addr_and_type = inp.next().ok_or("Give address, optionally give a type")?;
            let (addr_str, typename) = match addr_and_type.split_once(':') {
                Some((addr, ty)) => (addr, ty),
                None => (addr_and_type, "i64"),
            };
            let addr: isize = addr_str.parse()?;
            let value_str = inp.next().ok_or("Expecting value to write")?;

            parsetype_and_poke!(
                value_str, typename, child_pid, addr, i32, u32, i64, u64, char, bool, u8, i8,
                usize, isize, i16, u16, f32, f64, i128, u128
            );
        }
        "locate" => {
            // locate <functionname>
            let function_name = inp.next().ok_or("Require functionname")?;
            let locations = target.dwinfo.function_addresses(function_name)?;
            if locations.is_empty() {
                println!("No locations found for {function_name}");
            }
            let base_address = target.get_base_address()?;
            for location in locations {
                let real_location = location + base_address as isize;
                println!("0x{location:x} + 0x{base_address:x} = {real_location:x}");
            }
        }
        _ => {
            println!("Dont know command: {command}");
        }
    };
    Ok(())
}
