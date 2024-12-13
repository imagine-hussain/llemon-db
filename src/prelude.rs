use fork;
use libc::{ptrace, PTRACE_PEEKDATA};

use crate::ptrace;
use std::{ffi::c_void, os::unix::process::CommandExt, process::Command};

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Pid(pub i32);

pub const NULLVOID: *const c_void = std::ptr::null::<c_void>();

pub fn launch_traceable(mut command: Command) -> Result<Pid, i32> {
    match fork::fork()? {
        fork::Fork::Parent(child_pid) => Ok(Pid(child_pid)),
        fork::Fork::Child => {
            ptrace::trace_me();
            // execute the other program (inplace)
            command.exec();
            panic!("Bro how did u fail to execute");
        }
    }
}

#[derive(Debug, Clone, Hash)]
pub struct Breakpoint {
    pid: Pid,
    addr: isize,
    enabled: bool,
    replacing_byte: Option<u8>,
}

impl Breakpoint {
    /// This is the interrupt instruction for x86.
    /// TODO: Allow arm as well using `cfg` flags
    pub const INT3_INSTRUCTION: u8 = 0xcc;

    pub fn enable(&mut self) {
        let word_at_addr = ptrace::peekdata(self.pid, self.addr);

        let mut bytes_at_addr = word_at_addr.to_le_bytes();

        let lowest_byte = unsafe { *bytes_at_addr.get_unchecked(0) };
        self.replacing_byte = Some(lowest_byte);

        // Replace the word_at the addr with an `INT3` and then put this memory back
        unsafe {
            *bytes_at_addr.get_unchecked_mut(0) = Self::INT3_INSTRUCTION;
        }

        let word_at_addr_with_int3 = i64::from_le_bytes(bytes_at_addr);
        ptrace::pokedata(self.pid, self.addr, word_at_addr_with_int3);

        self.enabled = true;
    }

    pub fn disable(&mut self) {
        assert!(self.enabled);
        let data = ptrace::peekdata(self.pid, self.addr);
        let old_byte = self.replacing_byte.take().expect("Was enabled");

        let mut bytes = data.to_le_bytes();
        unsafe {
            *bytes.get_unchecked_mut(0) = old_byte;
        }
        let new_data = i64::from_le_bytes(bytes);
        ptrace::pokedata(self.pid, self.addr, new_data);
    }
}

// R
// [P, C]
//
