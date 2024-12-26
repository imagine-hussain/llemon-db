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

pub fn has_errno() -> bool {
    unsafe { *libc::__errno_location() == 0 }
}

pub fn trace_me() {
    unsafe {
        libc::ptrace(libc::PTRACE_TRACEME, 0, 0, NULLVOID);
    };
}

pub fn peekdata(pid: Pid, addr: u64) -> Result<i64, Error> {
    clear_errno();
    let data = unsafe { libc::ptrace(libc::PTRACE_PEEKDATA, pid.0, addr, NULLVOID) };
    match check_errno() {
        None => Ok(data),
        Some(err) => Err(err),
    }
}


pub fn peekdata_slice(pid: Pid, mut addr: u64, data: &mut [u8]) -> Result<(), Error> {
    let mut remaining = data;

    while !remaining.is_empty() {
        let word_at_addr: i64 = peekdata(pid, addr)?;
        let mut word_as_slice: [u8; 8] = word_at_addr.to_ne_bytes();

        let bytes_to_copy = remaining.len().min(8);
        let src_buf = &mut word_as_slice[..bytes_to_copy];

        for (dest, src) in remaining.iter_mut().zip(src_buf) {
            *dest = *src;
        }

        remaining = &mut remaining[bytes_to_copy..];
        addr += bytes_to_copy as u64;
    }

    Ok(())
}

pub fn peekdata_as<T: Sized>(pid: Pid, addr: u64) -> Result<T, Error> {
    let len: usize = std::mem::size_of::<T>();
    let mut t = MaybeUninit::<T>::uninit();
    
    // # Safety:
    // The pointer for `from_raw_parts_mut` is constructed off a local
    // Sized variable and is valid.
    unsafe {
        let data: &mut [u8] = std::slice::from_raw_parts_mut(t.as_mut_ptr() as *mut u8, len);
        peekdata_slice(pid, addr, data)?;
    };
    
    unsafe {
        Ok(t.assume_init())
    }
}


pub fn pokedata(pid: Pid, addr: u64, data: i64) -> Result<(), Error> {
    clear_errno();
    let res = unsafe { libc::ptrace(libc::PTRACE_POKEDATA, pid, addr, data) };
    match res {
        -1 => Err(check_errno().unwrap()),
        _ => Ok(()),
    }
}


pub fn pokedata_slice(pid: Pid, mut addr: u64, data: &[u8]) -> Result<(), Error> {
    let mut remaining = data;

    while !remaining.is_empty() {
        let bytes_to_copy = remaining.len().min(8);

        if bytes_to_copy == 8 {
            pokedata(pid, addr, remaining.as_ptr() as i64)?;
        }
        else {
            let word_at_addr: i64 = peekdata(pid, addr)?;
            let mut word_as_slice: [u8; 8] = word_at_addr.to_ne_bytes();

            let dest_buf = &mut word_as_slice[..bytes_to_copy];

            for (src, dest) in remaining.iter().zip(dest_buf) {
                *dest = *src;
            }

            let written_data = i64::from_ne_bytes(word_as_slice);
            pokedata(pid, addr, written_data)?;
        }

        remaining = &remaining[bytes_to_copy..];
        addr += bytes_to_copy as u64;
    }

    Ok(())
}

pub fn pokedata_as<T: Sized>(pid: Pid, addr: u64, data: &T) -> Result<(), Error> {
    let len = std::mem::size_of::<T>();
    let data_slice = unsafe {
        let underlying = data as *const T as *const u8;
        std::slice::from_raw_parts(underlying, len)
    };

    pokedata_slice(pid, addr, data_slice)?;
    Ok(())
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

pub fn single_step(pid: Pid) -> Result<(), Error> {
    unsafe {
        let res = libc::ptrace(libc::PTRACE_SINGLESTEP, pid.0, NULLVOID, NULLVOID);
        match res {
            -1 => Err(check_errno().unwrap()),
            _ => Ok(()),
        }
    }
}
