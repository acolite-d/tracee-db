use super::symbol::*;
use crate::traceedb::breakpoint::*;
use crate::traceedb::command::*;

use gimli::Dwarf;
use nix::{
    sys::ptrace,
    sys::signal::{kill, Signal},
    sys::wait::{waitpid, WaitStatus},
    unistd::{execv, fork, ForkResult, Pid},
};

use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::{c_void, CStr, CString};
use std::io::Write;
use std::io::{stdin, stdout};
use std::{borrow, ops::Index};

#[derive(Debug)]
pub struct TraceeDbg<'dwarf> {
    program: Option<String>,
    breakpoints: RefCell<HashMap<u64, BrkptRecord>>,
    symbols: Option<RefCell<Dwarf<borrow::Cow<'dwarf, [u8]>>>>,
}

impl<'dwarf> TraceeDbg<'dwarf> {
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

        if self.symbols.is_none() {
            println!("WARNING: No debug symbols loaded!")
        }

        'await_process: loop {
            let wait_status = waitpid(target_pid, None);

            'await_user: loop {
                match wait_status {
                    Ok(WaitStatus::Stopped(_, Signal::SIGTRAP))
                    | Ok(WaitStatus::Stopped(_, Signal::SIGSTOP)) => {
                        // First, check to see if the place where we stopped has an
                        // associated breakpoint. If PC == BPT_PC, then replace trap,
                        // rollback PC, and proceed.

                        let mut regs =
                            ptrace::getregs(target_pid).expect("FATAL: failed to send PTRACE_REGS");

                        // self.breakpoints.borrow().get(&regs.rip).map(|brkpt| brkpt.recover_from_trap());

                        dbg!(self.breakpoints.borrow());

                        if let Some(breakpoint) = self.breakpoints.borrow().get(&regs.rip) {
                            // unsafe {
                            //     poke_text(
                            //         target_pid,
                            //         breakpoint.pc_addr,
                            //         breakpoint.original_insn as *mut c_void,
                            //     )
                            //     .expect("failed to write to .text section with PTRACE_POKETEXT");
                            // }
                            // regs.rip -= 1;
                            // ptrace::setregs(target_pid, regs).expect("FATAL: Failed to set regs");
                        }

                        match self
                            .prompt_user_cmd()
                            .and_then(|cmd| cmd.execute(target_pid))
                        {
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

                            Ok(TargetStat::BreakpointAdded(brkptrec)) => {
                                println!("Breakpoint added: {:#x}", brkptrec.pc_addr as u64);
                                continue 'await_user;
                            }

                            Err(err_msg) => {
                                eprintln!("Err: {}", err_msg);
                                continue;
                            }
                        }
                    }

                    Ok(WaitStatus::Stopped(_, Signal::SIGSEGV)) => {
                        println!("Target process received SIGSEGV, segfaulted!");
                        break 'await_process;
                    }

                    Ok(WaitStatus::Exited(_, ..)) => {
                        println!("The target program finished execution.");
                        break 'await_process;
                    }

                    Ok(_unhandled) => {
                        dbg!(_unhandled);
                        todo!();
                    }

                    Err(_) => {
                        panic!("Critical failure: failed to wait for target program!");
                    }
                }
            }
        }
    }

    fn prompt_user_cmd(&self) -> Result<Box<dyn Execute>, &'static str> {
        print!("> ");
        stdout().flush().unwrap();

        let mut user_input = String::new();

        while let Err(_) = stdin().read_line(&mut user_input) {
            eprintln!("Err: Failed to read user input, please enter a proper command!");
            user_input.clear();
        }

        let mut term_iter = user_input.split_whitespace();

        let (command, mut args_iter) = (term_iter.nth(0).unwrap(), term_iter);

        match command {
            // Commands with no operands
            "reg" | "registers" => Ok(Box::new(ViewRegisters)),
            "s" | "step" => Ok(Box::new(Step)),
            "c" | "continue" => Ok(Box::new(Continue)),
            "q" | "quit" => Ok(Box::new(Quit)),
            "h" | "help" => Ok(Box::new(HelpMe)),

            // Commands with a single operand
            "r" | "read" => {
                let result = args_iter
                    .nth(0)
                    .ok_or("Missing the address to read from")
                    .and_then(|arg| {
                        usize::from_str_radix(arg, 16)
                            .map_err(|_| "Failed to parse: please supply hex value!")
                    });

                match result {
                    Ok(addr) => Ok(Box::new(ReadWord {
                        addr: addr as *mut c_void,
                    })),

                    Err(str) => Err(str),
                }
            }

            // Commands with two operands
            "w" | "write" => {
                let mut res = args_iter
                    .take(2)
                    .map(|arg| usize::from_str_radix(arg, 16));

                match (res.next(), res.next()) {
                    (Some(Ok(addr)), Some(Ok(val))) => Ok(Box::new(WriteWord {
                        addr: addr as *mut c_void,
                        val: val as *mut c_void,
                    })),

                    _ => Err("Failed to parse args for writing word!"),
                }
            }
            "b" | "breakpoint" => {
                if let Some(ref symref) = self.symbols {
                    let res = args_iter
                        .next()
                        .as_deref()
                        .ok_or("Insufficient arguments for command!")
                        .and_then(|arg| parse_file_and_lineno(arg))
                        .and_then(|(fname, lno)| {
                            src_line_to_addr(symref.borrow(), fname, lno)
                                .map_err(|_| "Failed to resolve address!")
                        });

                    let _ = res.map(|addr| println!("{:#x}", addr));

                    match res {
                        Ok(addr) => Ok(Box::new(Breakpoint(addr))),
                        Err(msg) => Err(msg),
                    }
                } else {
                    Err("Cannot resolve source lines without debug symbols!")
                }
            }

            _ => Err("Could not recognize command!"),
        }
    }
}

fn parse_file_and_lineno(string: &str) -> Result<(&str, u64), &'static str> {
    let vec: Vec<&str> = string.split(':').collect();

    if vec.len() != 2 {
        return Err("Failed to parse, please supply in format of file:lineno");
    }

    if let Ok(lineno) = vec.index(1).parse::<u64>() {
        return Ok((vec[0], lineno));
    } else {
        return Err("Failed to parse a line number from supplied argument!");
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
        self.symbols = load_dwarf_data(file_buf).ok();
        self
    }

    pub fn build(self) -> TraceeDbg<'dwarf> {
        TraceeDbg {
            program: self.program,
            breakpoints: RefCell::new(HashMap::default()),
            symbols: match self.symbols {
                Some(sym) => Some(RefCell::new(sym)),
                None => None,
            },
        }
    }
}

pub fn run_get_pid_dialogue() -> Pid {
    let mut input = String::new();
    let mut pid: Result<Pid, &str>;

    loop {
        print!("Please enter a target PID: ");
        stdout().flush().unwrap();

        stdin()
            .read_line(&mut input)
            .expect("Failed to read in line from input!");

        pid = input.as_str()
            .trim()
            .parse::<i32>()
            .map(Pid::from_raw)
            .map_err(|_| "Please supply an integer for PID!")
            .and_then(|pid| {
                if let Ok(_) = kill(pid, None) {
                    Ok(pid)
                } else {
                    Err("Process does not exist!")
                }
            });
            
        if let Ok(_) = pid { break; }

        input.clear();
    }

    pid.unwrap()
}
