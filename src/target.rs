use std::collections::HashMap;
use std::io::BufRead;
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

    pub fn read_word(&mut self, addr: isize) -> Result<i64, ptrace::Error> {
        ptrace::peekdata(self.pid, addr)
    }

    pub fn write_word(&mut self, addr: isize, data: i64) -> Result<(), ptrace::Error> {
        ptrace::pokedata(self.pid, addr, data)
    }

    fn wait_signal(&self) {
        wait_for_signal(self.pid);
    }

    pub fn pid(&self) -> Pid {
        self.pid
    }

    pub fn kill(&mut self) -> Result<(), &'static str> {
        unsafe {
            // TODO: Error check here, this is fallible
            match libc::kill(self.pid.0, libc::SIGKILL) {
                -1 => Err("Could not kill process"),
                0 => Ok(()),
                _ => unreachable!("libc should only return 0 or -1 for lib::kill"),
            }
        }
    }

    // TODO: This should be cached to avoid reopening this file
    pub fn get_base_address(&mut self) -> std::io::Result<u64> {
        // Open the /proc/[pid]/maps file
        let path = format!("/proc/{}/maps", self.pid.0);
        let file = std::fs::File::open(path)?;

        // Read the file line by line
        let mut bufreader = std::io::BufReader::new(file);
        for line in bufreader.lines() {
            let line = line?;

            // The base address is in the first column and is the first part of the line
            if let Some(address_str) = line.split_whitespace().next() {
                if let Ok(address) = u64::from_str_radix(address_str.split('-').next().unwrap(), 16) {
                    return Ok(address);
                }
            }
        }

        // If no base address is found, return an error
        Err(std::io::Error::new(std::io::ErrorKind::NotFound, "Base address not found in mapping file"))
    }}
