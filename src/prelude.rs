use crate::{ptrace, target::Target};
use crate::dwarf;

use std::{ffi, os::unix::process::CommandExt, process};

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Pid(pub i32);

pub const NULLVOID: *const ffi::c_void = std::ptr::null::<ffi::c_void>();

pub fn launch_traceable(mut command: process::Command) -> Result<Target, i32> {
    match fork::fork()? {
        fork::Fork::Parent(child_pid) => {
            let program = command.get_program();
            let dwarf = dwarf::read_dwarf(program.to_str().unwrap()).unwrap();
            let dwinfo = dwarf::DwarfInfo::new(dwarf);
            Ok(Target::new(Pid(child_pid), dwinfo))
        },
    fork::Fork::Child => {
            ptrace::trace_me();
            // execute the other program (inplace)
            command.exec();
            panic!("Bro how did u fail to execute");
        }
    }
}

pub fn wait_for_signal(pid: Pid) {
    let mut status: i32 = 0;
    let options: i32 = 0;
    unsafe {
        libc::waitpid(pid.0, &mut status as *mut i32, options);
    }
}

#[macro_export]
macro_rules! peektype_and_print {
    ($typename:expr, $pid:expr, $addr:expr, $($ty:ty), *) => {
        {
            let pid = $pid;
            let addr = $addr;

            match $typename {
                $(
                    stringify!($ty) => {
                        let val: $ty = ptrace::peekdata_as(pid, addr).unwrap();
                        println!("{val}");
                    }
                ),*
                provided_typename => {
                    println!("Invalid typename: {}", provided_typename);
                }
            }
        }
    };

}

#[macro_export]
macro_rules! parsetype_and_poke {
    ($value_str:expr, $typename:expr, $pid:expr, $addr:expr, $($ty:ty), *) => {
        {
            let pid = $pid;
            let addr = $addr;
            let value_str = $value_str;

            match $typename {
                $(
                    stringify!($ty) => {
                        let val: $ty = value_str.parse().unwrap();
                        ptrace::pokedata_as(pid, addr, &val)?;
                        println!("Succesfully wrote to {}", addr);
                    }
                ),*
                provided_typename => {
                    println!("Invalid typename: {}", provided_typename);
                }
            }
        }
    };

}
