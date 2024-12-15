use crate::{ptrace, target::Target};

use std::{ffi, os::unix::process::CommandExt, process};

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Pid(pub i32);

pub const NULLVOID: *const ffi::c_void = std::ptr::null::<ffi::c_void>();

pub fn launch_traceable(mut command: process::Command) -> Result<Target, i32> {
    match fork::fork()? {
        fork::Fork::Parent(child_pid) => Ok(Target::from_pid(Pid(child_pid))),
        fork::Fork::Child => {
            ptrace::trace_me();
            // execute the other program (inplace)
            command.exec();
            panic!("Bro how did u fail to execute");
        }
    }
}

pub fn wait_for_signal(pid: Pid) {
    let mut status: i32 = 0;
    let options: i32 = 0;
    unsafe {
        libc::waitpid(pid.0, &mut status as *mut i32, options);
    }
}
