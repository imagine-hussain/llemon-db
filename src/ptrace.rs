use core::panic;
use std::fmt::Display;
use std::mem::MaybeUninit;

use crate::prelude::*;
use crate::registers::Register;

use libc;
// The things in this module should check `errno`

#[derive(Copy, Clone, Debug)]
pub enum Error {
    NoSuchProcess = libc::ESRCH as isize,
    EIO = libc::EIO as isize,
}

impl std::error::Error for Error {}

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
    unsafe { *libc::__errno_location() == 0 }
}

pub fn trace_me() {
    unsafe {
        libc::ptrace(libc::PTRACE_TRACEME, 0, 0, NULLVOID);
    };
}

pub fn peekdata(pid: Pid, addr: isize) -> Result<i64, Error> {
    clear_errno();
    let data = unsafe { libc::ptrace(libc::PTRACE_PEEKDATA, pid.0, addr, NULLVOID) };
    match check_errno() {
        None => Ok(data),
        Some(err) => Err(err),
    }
}

pub fn pokedata(pid: Pid, addr: isize, data: i64) -> Result<(), Error> {
    clear_errno();
    let res = unsafe { libc::ptrace(libc::PTRACE_POKEDATA, pid, addr, data) };
    match res {
        -1 => Err(check_errno().unwrap()),
        _ => Ok(()),
    }
}

pub fn cont(pid: Pid) -> Result<(), Error> {
    let res = unsafe { libc::ptrace(libc::PTRACE_CONT, pid.0, NULLVOID, NULLVOID) };

    match res {
        -1 => Err(check_errno().unwrap()),
        _ => Ok(()),
    }
}

pub fn get_regs(pid: Pid) -> Result<libc::user_regs_struct, Error> {
    unsafe {
        let mut regs = MaybeUninit::<libc::user_regs_struct>::uninit();
        let res = libc::ptrace(libc::PTRACE_GETREGS, pid.0, NULLVOID, regs.as_mut_ptr());
        match res {
            -1 => Err(check_errno().unwrap()),
            _ => Ok(regs.assume_init()),
        }
    }
}

pub fn set_regs(pid: Pid, regs: &libc::user_regs_struct) -> Result<(), Error> {
    unsafe {
        let r = regs as *const _;
        let res = libc::ptrace(libc::PTRACE_SETREGS, pid.0, NULLVOID, r);
        match res {
            -1 => Err(check_errno().unwrap()),
            _ => Ok(()),
        }
    }
}

pub fn set_reg(pid: Pid, reg: Register, value: u64) -> Result<(), Error> {
    let mut regs = get_regs(pid)?;

    *reg.extract_mut_from_reg_struct(&mut regs) = value;

    set_regs(pid, &regs)
}

pub fn get_reg(pid: Pid, reg: Register) -> Result<u64, Error> {
    let regs = get_regs(pid)?;
    Ok(*reg.extract_from_reg_struct(&regs))
}

impl From<i32> for Error {
    fn from(value: i32) -> Self {
        match value {
            libc::EIO => Self::EIO,
            libc::ESRCH => Self::EIO,
            e => panic!("Not a handled error code for ptrace: {e}"),
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NoSuchProcess => write!(f, "NoSuchProcess"),
            Error::EIO => write!(f, "EIO"),
        }
    }
}
