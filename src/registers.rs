use std::str::{self, FromStr};

use libc::c_ulonglong;

/// All the register types from `libc::user_regs_struct`
/// Their values map to the index in `libc::user_regs_struct`
#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum Register {
    R15 = 0,
    R14 = 1,
    R13 = 2,
    R12 = 3,
    RBP = 4,
    RBX = 5,
    R11 = 6,
    R10 = 7,
    R9 = 8,
    R8 = 9,
    RAX = 10,
    RCX = 11,
    RDX = 12,
    RSI = 13,
    RDI = 14,
    ORIGRAX = 15,
    RIP = 16,
    CS = 17,
    RFLAGS = 18,
    RSP = 19,
    SS = 20,
    FSBASE = 21,
    GSBASE = 22,
    DS = 23,
    ES = 24,
    FS = 25,
    GS = 26,
}

impl Register {
    pub const NUM_VARIANTS: usize = 27;

    pub const fn pc() -> Self {
        Self::RIP
    }

    fn dwarf(&self) -> i32 {
        match self {
            Self::R15 => 15,
            Self::R14 => 14,
            Self::R13 => 13,
            Self::R12 => 12,
            Self::RBP => 6,
            Self::RBX => 3,
            Self::R11 => 11,
            Self::R10 => 10,
            Self::R9 => 9,
            Self::R8 => 8,
            Self::RAX => 0,
            Self::RCX => 2,
            Self::RDX => 1,
            Self::RSI => 4,
            Self::RDI => 5,
            Self::ORIGRAX => -1,
            Self::RIP => -1,
            Self::CS => 51,
            Self::RFLAGS => 49,
            Self::RSP => 7,
            Self::SS => 52,
            Self::FSBASE => 58,
            Self::GSBASE => 59,
            Self::DS => 53,
            Self::ES => 50,
            Self::FS => 54,
            Self::GS => 55,
        }
    }

    fn from_dwarf(dwarf: i32) -> Option<Self> {
        let reg = match dwarf {
            15 => Self::R15,
            14 => Self::R14,
            13 => Self::R13,
            12 => Self::R12,
            6 => Self::RBP,
            3 => Self::RBX,
            11 => Self::R11,
            10 => Self::R10,
            9 => Self::R9,
            8 => Self::R8,
            0 => Self::RAX,
            2 => Self::RCX,
            1 => Self::RDX,
            4 => Self::RSI,
            5 => Self::RDI,
            51 => Self::CS,
            49 => Self::RFLAGS,
            7 => Self::RSP,
            52 => Self::SS,
            58 => Self::FSBASE,
            59 => Self::GSBASE,
            53 => Self::DS,
            50 => Self::ES,
            54 => Self::FS,
            55 => Self::GS,
            _ => return None,
        };
        Some(reg)
    }

    pub fn extract_from_reg_struct<'a>(&self, regs: &'a libc::user_regs_struct) -> &'a u64 {
        let regs = regs as *const libc::user_regs_struct;
        let regs_raw_slice = regs as *const c_ulonglong;
        unsafe {
            let offset = *self as u8;
            regs_raw_slice.offset(offset as isize).as_ref().unwrap()
        }
    }

    pub fn extract_mut_from_reg_struct<'a>(
        &self,
        regs: &'a mut libc::user_regs_struct,
    ) -> &'a mut u64 {
        let regs = regs as *mut libc::user_regs_struct;
        let regs_raw_slice = regs as *mut c_ulonglong;
        unsafe {
            let offset = *self as u8;
            regs_raw_slice.add(offset as usize).as_mut().unwrap()
        }
    }

    /// # Safety
    /// Ensure that val in [0..Self::NUM_VARIANTS), else this will
    /// have a malformed value
    pub unsafe fn from_u8_unchecked(val: u8) -> Self {
        std::mem::transmute(val)
    }
}

impl FromStr for Register {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "R15" | "r15" => Ok(Self::R15),
            "R14" | "r14" => Ok(Self::R14),
            "R13" | "r13" => Ok(Self::R13),
            "R12" | "r12" => Ok(Self::R12),
            "RBP" | "rbp" => Ok(Self::RBP),
            "RBX" | "rbx" => Ok(Self::RBX),
            "R11" | "r11" => Ok(Self::R11),
            "R10" | "r10" => Ok(Self::R10),
            "R9" | "r9" => Ok(Self::R9),
            "R8" | "r8" => Ok(Self::R8),
            "RAX" | "rax" => Ok(Self::RAX),
            "RCX" | "rcx" => Ok(Self::RCX),
            "RDX" | "rdx" => Ok(Self::RDX),
            "RSI" | "rsi" => Ok(Self::RSI),
            "RDI" | "rdi" => Ok(Self::RDI),
            "ORIGRAX" | "origrax" => Ok(Self::ORIGRAX),
            "RIP" | "rip" => Ok(Self::RIP),
            "CS" | "cs" => Ok(Self::CS),
            "RFLAGS" | "rflags" => Ok(Self::RFLAGS),
            "RSP" | "rsp" => Ok(Self::RSP),
            "SS" | "ss" => Ok(Self::SS),
            "FSBASE" | "fsbase" => Ok(Self::FSBASE),
            "GSBASE" | "gsbase" => Ok(Self::GSBASE),
            "DS" | "ds" => Ok(Self::DS),
            "ES" | "es" => Ok(Self::ES),
            "FS" | "fs" => Ok(Self::FS),
            "GS" | "gs" => Ok(Self::GS),
            _ => Err("No such register"),
        }
    }
}

pub fn dump_user_regs(regs: &libc::user_regs_struct) {
    let regs = regs as *const libc::user_regs_struct;
    let regs_raw_slice = regs as *const c_ulonglong;

    for i in 0..Register::NUM_VARIANTS {
        unsafe {
            let regkind = Register::from_u8_unchecked(i as u8);
            let reg_value = *regs_raw_slice.add(i);

            println!("{:?}: {} = 0x{:x}", regkind, reg_value, reg_value)
        }
    }
}

use std::convert::TryFrom;

impl TryFrom<u8> for Register {
    type Error = &'static str;

    fn try_from(val: u8) -> Result<Self, Self::Error> {
        if val as usize >= Self::NUM_VARIANTS {
            return Err("Val is out of range for register");
        }

        // Safety: Checked that we have a valid tag in range
        unsafe { Ok(Self::from_u8_unchecked(val)) }
    }
}
