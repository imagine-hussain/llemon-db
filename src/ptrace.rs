use libc::{self, ptrace};

pub fn trace_me() {
    unsafe {
        libc::ptrace(libc::PTRACE_TRACEME);
        let i = 8;
    }
}
