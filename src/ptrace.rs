use crate::prelude::*;

use libc;
// The things in this module should check `errno`

pub fn trace_me() {
    unsafe {
        libc::ptrace(libc::PTRACE_TRACEME, 0, 0, NULLVOID);
    };
}

pub fn peekdata(pid: Pid, addr: isize) -> i64 {
    unsafe { libc::ptrace(libc::PTRACE_PEEKDATA, pid.0, addr, NULLVOID) }
}

pub fn pokedata(pid: Pid, addr: isize, data: i64) {
    unsafe {
        libc::ptrace(libc::PTRACE_POKEDATA, pid, addr, data);
    }
}

// hello
