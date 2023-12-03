mod traceedb;
use traceedb::function::{run_debugger, run_target, run_get_pid_dialogue};

use nix::{
    sys::ptrace,
    unistd::{fork, ForkResult},
};

use std::ffi::CString;
use std::env;

fn main() {
    println!(
        "NEXTDB DEBUGGER\nType \"help\" for command list!"
    );

    let target_program = env::args().map(|arg| CString::new(arg).unwrap()).nth(1);

    if let Some(ref prog_name) = target_program {
        match unsafe { fork() } {
            Ok(ForkResult::Parent { child, .. }) => {
                println!("Spawned child process {}", child);
                run_debugger(child);
            }

            Ok(ForkResult::Child) => {
                run_target(prog_name.as_c_str());
            }

            Err(_) => panic!("Failed to fork process, exiting..."),
        }
    } else {
        let target_pid = run_get_pid_dialogue();

        ptrace::attach(target_pid).expect("Failed to attach to running process!");
        run_debugger(target_pid);
    }
}