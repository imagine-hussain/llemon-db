use crate::breakpoint::Breakpoint;
use crate::ptrace;
use std::{collections::HashMap, ffi::c_void, os::unix::process::CommandExt, process::Command};

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

pub struct Debugger {
    pid: Pid,
    breakpoints: HashMap<isize, Breakpoint>,
}

impl Debugger {
    pub fn from_pid(pid: Pid) -> Self {
        Self {
            pid,
            breakpoints: HashMap::default(),
        }
    }

    pub fn add_breakpoint_at(&mut self, addr: isize) -> Result<(), ptrace::Error> {
        let breakpoint = self
            .breakpoints
            .entry(addr)
            .or_insert_with(|| Breakpoint::new(self.pid, addr));

        breakpoint.enable()
    }

    pub fn continue_process(&mut self) {
        ptrace::cont(self.pid);

        let mut status: i32 = 0;
        let res = unsafe { libc::waitpid(self.pid.0, &mut status, 0) };
        match res {
            -1 => panic!("Can't wait on pid"),
            _ => (),
        };
    }
    // void debugger::continue_execution() {
    //     ptrace(PTRACE_CONT, m_pid, nullptr, nullptr);

    //     int wait_status;
    //     auto options = 0;
    //     waitpid(m_pid, &wait_status, options);
    // }
}

// R
// [P, C]
//
