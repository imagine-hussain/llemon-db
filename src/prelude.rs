use fork;

use crate::ptrace;
use std::{os::unix::process::CommandExt, process::Command};

pub fn launch_traceable(mut command: Command) -> Result<i32, i32> {
    // -> never
    // -> bool
    // ls -l
    //

    //
    // let x = match T {
    //  => never
    //  => i32
    //}
    //
    // FOrk

    match fork::fork()? {
        fork::Fork::Parent(child_pid) => Ok(child_pid),
        fork::Fork::Child => {
            ptrace::trace_me();
            // execute the other program (inplace)
            command.exec();
            panic!("Bro how did u fail to execute");
        }
    }
}
