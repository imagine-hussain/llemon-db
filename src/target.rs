use std::collections::HashMap;

use crate::breakpoint::Breakpoint;
use crate::prelude::*;
use crate::ptrace;
use crate::registers::Register;

pub struct Target {
    pid: Pid,
    breakpoints: HashMap<isize, Breakpoint>,
}

impl Target {
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

    pub fn step_over_breakpoint(&mut self) -> Result<(), ptrace::Error> {
        let candidate_breakpoint_addr = ptrace::get_reg(self.pid, Register::pc())? as isize;

        let Some(bp) = self.breakpoints.get_mut(&candidate_breakpoint_addr) else {
            return Ok(());
        };
        if !bp.enabled() {
            return Ok(());
        }

        // Go back to the where the INT3 breakpoint was and restore it
        ptrace::set_reg(self.pid, Register::pc(), candidate_breakpoint_addr as u64)?;
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

    pub fn pid(&self) -> Pid {
        self.pid
    }
}
