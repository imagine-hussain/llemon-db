use crate::prelude::*;
use crate::ptrace;

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
    pub const INT3_INSTRUCTION: u8 = 0xCC;

    pub fn new(pid: Pid, addr: isize) -> Self {
        Self {
            pid,
            addr,
            enabled: false,
            replacing_byte: None,
        }
    }

    pub fn new_enabled(pid: Pid, addr: isize) -> Result<Self, ptrace::Error> {
        let mut breakpoint = Self::new(pid, addr);
        breakpoint.enable()?;
        Ok(breakpoint)
    }

    pub fn enable(&mut self) -> Result<(), ptrace::Error> {
        if self.enabled {
            // This should signal that this already exists
            return Ok(());
        }

        let word_at_addr = ptrace::peekdata(self.pid, self.addr)?;

        let mut bytes_at_addr = word_at_addr.to_le_bytes();

        println!(
            "Setting break at {:x} with data {:x}",
            self.addr, word_at_addr
        );
        let lowest_byte = unsafe { *bytes_at_addr.get_unchecked(0) };
        self.replacing_byte = Some(lowest_byte);

        // Replace the word_at the addr with an `INT3` and then put this memory back
        unsafe {
            *bytes_at_addr.get_unchecked_mut(0) = Self::INT3_INSTRUCTION;
        }

        let word_at_addr_with_int3 = i64::from_le_bytes(bytes_at_addr);
        ptrace::pokedata(self.pid, self.addr, word_at_addr_with_int3)?;

        self.enabled = true;

        Ok(())
    }

    pub fn disable(&mut self) -> Result<(), ptrace::Error> {
        assert!(self.enabled);
        let data = ptrace::peekdata(self.pid, self.addr)?;
        let old_byte = self.replacing_byte.take().expect("Was enabled");

        let mut bytes = data.to_le_bytes();
        unsafe {
            *bytes.get_unchecked_mut(0) = old_byte;
        }
        let new_data = i64::from_le_bytes(bytes);
        ptrace::pokedata(self.pid, self.addr, new_data)?;

        Ok(())
    }
}
