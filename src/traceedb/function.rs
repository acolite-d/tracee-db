use super::symbol;
use crate::traceedb::command::*;

use gimli::Dwarf;
use nix::{
    errno::Errno,
    libc,
    sys::ptrace,
    sys::signal::{kill, Signal},
    sys::wait::{waitpid, WaitStatus},
    unistd::{execv, fork, ForkResult, Pid},
};

use std::borrow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::{c_long, c_uint, c_void, CStr, CString};
use std::io::Write;
use std::io::{stdin, stdout};

#[derive(PartialEq, Debug)]
struct BrkptRecord {
    pc_addr: *mut c_void,
    original_insn: *mut c_void,
}

impl BrkptRecord {
    pub fn new(
        pc_addr: *mut c_void, 
        original_insn: *mut c_void
    ) -> Self {
        Self { pc_addr, original_insn }
    }
}

#[derive(Debug)]
pub struct TraceeDb<'dwarf> {
    program: Option<String>,
    breakpoints: RefCell<HashMap<u64, BrkptRecord>>,
    symbols: RefCell<Option<Dwarf<borrow::Cow<'dwarf, [u8]>>>>,
}

impl<'dwarf> TraceeDb<'dwarf> {
    pub fn builder() -> TraceeBuilder<'dwarf> {
        TraceeBuilder::default()
    }

    pub fn run(mut self) {
        if self.program.is_some() {
            match unsafe { fork() } {
                Ok(ForkResult::Parent { child, .. }) => {
                    println!("Spawned child process {}", child);
                    self.run_debugger(child);
                }

                Ok(ForkResult::Child) => {
                    let prog_cstr = CString::new(self.program.take().unwrap()).unwrap();
                    self.run_target(prog_cstr.as_c_str());
                }

                Err(_) => panic!("Failed to fork process, exiting..."),
            }
        } else {
            let target_pid = run_get_pid_dialogue();

            ptrace::attach(target_pid).expect("Failed to attach to running process!");
            self.run_debugger(target_pid);
        }
    }

    fn run_target(self, prog_name: &CStr) {
        println!("Running traceable target program {:?}", prog_name);

        ptrace::traceme().expect("Ptrace failed, cannot debug!");
        let _ = execv(prog_name, &[] as &[&CStr]).expect("Failed to spawn process");
    }

    fn run_debugger(self, target_pid: Pid) {
        println!("Entering debugging loop...");

        'await_process: loop {
            let wait_status = waitpid(target_pid, None);

            let mut regs =
                ptrace::getregs(target_pid).expect("FATAL: failed to send PTRACE_REGS");

            if let Some(breakpoint) = self.breakpoints.borrow().get(&regs.rip) {
                unsafe { 
                    poke_text(target_pid, breakpoint.pc_addr, breakpoint.original_insn)
                        .expect("failed to write to .text section with PTRACE_POKETEXT"); 
                }
                regs.rip -= 1;
                ptrace::setregs(target_pid, regs).expect("FATAL: Failed to set regs");
            }

            'await_user: loop {
                match wait_status {
                    Ok(WaitStatus::Stopped(_, Signal::SIGTRAP))
                    | Ok(WaitStatus::Stopped(_, Signal::SIGSTOP)) => {
                        // First, check to see if the place where we stopped has an
                        // associated breakpoint. If PC == BPT_PC, then replace trap,
                        // rollback PC, and proceed.

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
}

#[derive(Default)]
pub struct TraceeBuilder<'dwarf> {
    program: Option<String>,
    symbols: Option<Dwarf<borrow::Cow<'dwarf, [u8]>>>,
}

impl<'dwarf> TraceeBuilder<'dwarf> {
    pub fn program(mut self, program: String) -> Self {
        self.program = Some(program);
        self
    }

    // pub fn initial_breakpt(mut self, initial_breakpt: Option<String>) -> Self {
    //     self.initial_breakpt = initial_breakpt;
    //     self
    // }

    pub fn dwarf_symbols(mut self, file_buf: &'dwarf [u8]) -> Self {
        self.symbols = symbol::load_dwarf_data(file_buf).ok();
        self
    }

    pub fn build(self) -> TraceeDb<'dwarf> {
        TraceeDb {
            program: self.program,
            breakpoints: RefCell::new(HashMap::default()),
            symbols: RefCell::new(self.symbols),
        }
    }
}

// pub unsafe fn write_breakpoint(target_pid: Pid, addr: *mut c_void) {
//     let insn = libc::ptrace(
//         ptrace::Request::PTRACE_PEEKTEXT as c_uint,
//         libc::pid_t::from(target_pid),
//         addr
//     );

//     libc::ptrace(
//         ptrace::Request::PTRACE_POKETEXT as c_uint,
//         libc::pid_t::from(target_pid),
//         addr,
//         0x0
//     );

//     ptrace::cont(target_pid, None);

//     libc::ptrace(
//         ptrace::Request::PTRACE_POKETEXT as c_uint,
//         libc::pid_t::from(target_pid),
//         addr,
//         insn
//     );

// }

pub unsafe fn poke_text(pid: Pid, addr: *mut c_void, val: *mut c_void) -> Result<(), &'static str> {
    libc::ptrace(
        ptrace::Request::PTRACE_POKETEXT as c_uint,
        libc::pid_t::from(pid),
        addr,
        val,
    );
    Ok(())
}

pub unsafe fn peek_text(pid: Pid, addr: *mut c_void) -> Result<(), &'static str> {
    libc::ptrace(
        ptrace::Request::PTRACE_PEEKTEXT as c_uint,
        libc::pid_t::from(pid),
        addr
    );
    Ok(())
}

pub fn print_help() {
    println!("List of Commands:");
    Step::help();
    Continue::help();
    ViewRegisters::help();
    ReadWord::help();
    WriteWord::help();
    Breakpoint::help();
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
