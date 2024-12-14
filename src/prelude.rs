use crate::ptrace::{self, set_reg};
use crate::registers::Register;
use crate::{breakpoint::Breakpoint, ptrace::get_reg};
use std::{
    borrow::BorrowMut, collections::HashMap, ffi::c_void, os::unix::process::CommandExt,
    process::Command,
};

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

    pub fn continue_process(&mut self) -> Result<(), ptrace::Error> {
        self.step_over_breakpoint()?;
        ptrace::cont(self.pid)?;

        self.wait_signal();
        Ok(())
    }

    fn step_over_breakpoint(&mut self) -> Result<(), ptrace::Error> {
        let candidate_breakpoint_addr = get_reg(self.pid, Register::pc())? as isize;

        let Some(bp) = self.breakpoints.get_mut(&candidate_breakpoint_addr) else {
            return Ok(());
        };
        if !bp.enabled() {
            return Ok(());
        }

        // Go back to the where the INT3 breakpoint was and restore it
        set_reg(self.pid, Register::pc(), candidate_breakpoint_addr as u64)?;
        bp.disable()?;

        ptrace::single_step(self.pid)?;
        self.wait_signal();
        self.breakpoints
            .get_mut(&candidate_breakpoint_addr)
            .expect("Will exist. Relooking up because of XOR lifetimes. TODO")
            .enable()?;

        Ok(())
    }

    fn wait_signal(&self) {
        wait_for_signal(self.pid);
    }
}

pub fn wait_for_signal(pid: Pid) {
    let mut status: i32 = 0;
    let options: i32 = 0;
    unsafe {
        libc::waitpid(pid.0, &mut status as *mut i32, options);
    }
}
