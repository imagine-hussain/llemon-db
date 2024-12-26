use std::collections::HashMap;
use std::fmt::Debug;
use std::io::BufRead;

use crate::breakpoint::Breakpoint;
use crate::dwarf::DwarfInfo;
use crate::prelude::*;
use crate::ptrace;
use crate::registers::Register;

pub struct Target {
    pub pid: Pid,
    pub breakpoints: HashMap<u64, Breakpoint>,
    pub base_address: Option<u64>,
    pub dwinfo: DwarfInfo,
    pub last_step_was_breakpoint: bool,
}

impl Target {
    pub fn new(pid: Pid, dwinfo: DwarfInfo) -> Self {
        Self {
            pid,
            breakpoints: HashMap::default(),
            base_address: None,
            dwinfo,
            last_step_was_breakpoint: false,
        }
    }

    pub fn add_breakpoint_at(&mut self, addr: u64) -> Result<(), ptrace::Error> {
        let breakpoint = self
            .breakpoints
            .entry(addr)
            .or_insert_with(|| Breakpoint::new(self.pid, addr));

        breakpoint.enable()
    }

    pub fn add_breakpoint_at_function(
        &mut self,
        function_name: &str,
    ) -> Result<u64, Box<dyn std::error::Error>> {
        let addresses = self.dwinfo.function_addresses(function_name)?;

        let base_address = self.get_base_address()?;
        for address in &addresses {
            self.add_breakpoint_at(*address + base_address)?;
        }

        Ok(addresses.len() as u64)
    }

    pub fn continue_process(&mut self) -> Result<(), ptrace::Error> {
        self.step_over_breakpoint()?;
        ptrace::cont(self.pid)?;

        self.wait_signal();
        Ok(())
    }

    pub fn step_instruction(&mut self) -> Result<(), ptrace::Error> {
        self.last_step_was_breakpoint = false;
        todo!()
    }

    pub fn step_over_breakpoint(&mut self) -> Result<(), ptrace::Error> {
        let current_pc = ptrace::get_reg(self.pid, Register::pc())?;
        dbg!(current_pc);
        let candidate_breakpoint_addr = current_pc - 1;

        let Some(bp) = self.breakpoints.get_mut(&candidate_breakpoint_addr) else {
            return Ok(());
        };

        if !bp.enabled() {
            return Ok(());
        }

        // Go back to the where the INT3 breakpoint was and restore it.
        ptrace::set_reg(self.pid, Register::pc(), candidate_breakpoint_addr)?;
        bp.disable()?;
        ptrace::single_step(self.pid)?;

        self.wait_signal();

        self.breakpoints
            .get_mut(&candidate_breakpoint_addr)
            .expect("Will exist. Relooking up because of XOR lifetimes. TODO")
            .enable()?;

        Ok(())
    }

    pub fn read_word(&mut self, addr: u64) -> Result<i64, ptrace::Error> {
        ptrace::peekdata(self.pid, addr)
    }

    pub fn write_word(&mut self, addr: u64, data: i64) -> Result<(), ptrace::Error> {
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

    pub fn get_base_address(&mut self) -> std::io::Result<u64> {
        if let Some(base_address) = self.base_address {
            return Ok(base_address);
        }

        // `/proc/[pid]/maps` contains the mappings for sections
        let path = format!("/proc/{}/maps", self.pid.0);
        let file = std::fs::File::open(path)?;

        let bufreader = std::io::BufReader::new(file);
        for line in bufreader.lines() {
            let line = line?;

            // The base address is in the first column and is the first part of the line
            if let Some(address_str) = line.split_whitespace().next() {
                if let Ok(address) = u64::from_str_radix(address_str.split('-').next().unwrap(), 16)
                {
                    self.base_address = Some(address);
                    return Ok(address);
                }
            }
        }

        // If no base address is found, return an error
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Base address not found in mapping file",
        ))
    }

    pub fn clear_base_address(&mut self) {
        self.base_address = None;
    }
}

impl Debug for Target {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Target {{ pid: {:?}, breakpoints: {:?} }}, ", self.pid, self.breakpoints)?;
        match self.base_address {
            None => write!(f, "Base address: None")?,
            Some(addr) => write!(f, "Base address: {:x?}", addr)?
        }

        Ok(())
    }
}
