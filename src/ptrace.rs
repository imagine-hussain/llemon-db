use core::panic;
use std::num::NonZero;

use crate::prelude::*;

use libc;
// The things in this module should check `errno`

#[derive(Copy, Clone, Debug)]
pub enum Error {
    NoSuchProcess = libc::ESRCH as isize,
    EIO = libc::EIO as isize,
}

fn check_errno() -> Option<Error> {
    let errno: i32 = unsafe { *libc::__errno_location() };
    match errno {
        0 => None,
        err => Some(err.into()),
    }
}

fn clear_errno() {
    unsafe {
        *libc::__errno_location() = 0;
    };
}

fn has_errno() -> bool {
    return unsafe { *libc::__errno_location() } == 0;
}

pub fn trace_me() {
    unsafe {
        libc::ptrace(libc::PTRACE_TRACEME, 0, 0, NULLVOID);
    };
}

pub fn peekdata(pid: Pid, addr: isize) -> Result<i64, Error> {
    let data = unsafe { libc::ptrace(libc::PTRACE_PEEKDATA, pid.0, addr, NULLVOID) };
    match check_errno() {
        None => Ok(data),
        Some(err) => Err(err),
    }
}

pub fn pokedata(pid: Pid, addr: isize, data: i64) {
    unsafe {
        libc::ptrace(libc::PTRACE_POKEDATA, pid, addr, data);
    }
}

pub fn cont(pid: Pid) {
    unsafe {
        libc::ptrace(libc::PTRACE_CONT, pid.0, NULLVOID, NULLVOID);
    }
}

// hello

impl From<i32> for Error {
    fn from(value: i32) -> Self {
        match value {
            libc::EIO => Self::EIO,
            libc::ESRCH => Self::EIO,
            e => panic!("Not a handled error code for ptrace: {e}"),
        }
    }
}
