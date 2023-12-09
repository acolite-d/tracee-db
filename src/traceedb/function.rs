use crate::traceedb::command::*;

use nix::{
    errno::Errno,
    libc::c_long,
    sys::ptrace,
    sys::signal::{kill, Signal},
    sys::wait::{waitpid, WaitStatus},
    unistd::{execv, fork, ForkResult, Pid},
};

use std::ffi::{c_void, CStr, CString};
use std::io::Write;
use std::io::{stdin, stdout};

#[derive(Debug)]
pub struct TraceeDb {
    target_exec: Option<CString>,
    initial_break: Option<CString>,
}

impl TraceeDb {
    pub fn builder() -> TraceeBuilder {
        TraceeBuilder::default()
    }

    pub fn run(self) {
        if let Some(ref prog_name) = self.target_exec {
            match unsafe { fork() } {
                Ok(ForkResult::Parent { child, .. }) => {
                    //

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
}

#[derive(Default)]
pub struct TraceeBuilder {
    target_exec: Option<CString>,
    initial_break: Option<CString>,
}

impl TraceeBuilder {
    pub fn target_exec(mut self, target_exec: Option<CString>) -> Self {
        self.target_exec = target_exec;
        self
    }

    pub fn initial_break(mut self, initial_break: Option<CString>) -> Self {
        self.initial_break = initial_break;
        self
    }

    pub fn build(self) -> TraceeDb {
        TraceeDb {
            target_exec: self.target_exec,
            initial_break: self.initial_break,
        }
    }
}

pub fn read_word(target_pid: Pid, addr: *mut c_void) -> Result<c_long, Errno> {
    ptrace::read(target_pid, addr)
}

pub fn write_word(target_pid: Pid, addr: *mut c_void, word: *mut c_void) -> Result<(), Errno> {
    unsafe { ptrace::write(target_pid, addr, word) }
}

pub fn print_help() {
    println!("List of Commands:");
    Step::help();
    Continue::help();
    ViewRegisters::help();
    ReadWord::help();
    WriteWord::help();
    Quit::help();
    HelpMe::help();
}

pub fn run_get_pid_dialogue() -> Pid {
    let mut input = String::new();
    let mut pid: Pid;

    loop {
        print!("Please enter a target PID: ");
        stdout().flush().unwrap();

        stdin()
            .read_line(&mut input)
            .expect("Did not enter correct string!");

        if let Ok(id) = input.trim().parse::<i32>() {
            pid = Pid::from_raw(id);

            if let Ok(_) = kill(pid, None) {
                break;
            } else {
                println!(
                    "Process PID {} does not exist, please enter an active process",
                    pid
                );
            }
        } else {
            println!("Please input a proper integer value for process ID!");
        }

        input.clear();
    }

    pid
}

pub fn run_target(prog_name: &CStr) {
    println!("Running traceable target program {:?}", prog_name);

    ptrace::traceme().expect("Ptrace failed, cannot debug!");
    let _ = execv(prog_name, &[] as &[&CStr]).expect("Failed to spawn process");
}

pub fn run_debugger(target_pid: Pid) {
    println!("Entering debugging loop...");

    'await_process: loop {
        let wait_status = waitpid(target_pid, None);

        'await_user: loop {

            match wait_status {
                Ok(WaitStatus::Stopped(_, Signal::SIGTRAP))
                | Ok(WaitStatus::Stopped(_, Signal::SIGSTOP)) => {
                    match prompt_user_cmd().and_then(|cmd| cmd.execute(target_pid)) {
                        Ok(TargetStat::AwaitingCommand) => {
                            continue 'await_user;
                        }

                        Ok(TargetStat::Running) => {
                            continue 'await_process;
                        }

                        Ok(TargetStat::Killed) => {
                            println!("Process killed, exiting...");
                            break 'await_process;
                        }

                        Err(err_msg) => {
                            eprintln!("Err: {}", err_msg);
                            continue;
                        }
                    }
                }

                Ok(WaitStatus::Stopped(_, Signal::SIGSEGV)) => {
                    println!("Child process received SIGSEGV, segfaulted!");
                    break;
                }

                Ok(WaitStatus::Exited(_, ..)) => {
                    println!("The target program finished execution.");
                    break;
                }

                Ok(_unhandled) => {
                    dbg!(_unhandled);
                    todo!();
                }

                Err(_) => {
                    panic!("failed to wait for target program!");
                }
            }
        }
    }
}
