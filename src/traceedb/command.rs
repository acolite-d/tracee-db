use crate::traceedb::{dbg::BrkptRecord, symbol::src_line_to_addr};

use super::dbg;

use nix::{sys::ptrace, unistd::Pid};

use gimli::Dwarf;
use std::borrow;
use std::cell::Ref;
use std::ffi::c_void;
use std::io::{stdin, stdout, Write};
use std::ops::Index;

pub enum TargetStat {
    AwaitingCommand,
    Running,
    Killed,
}

pub trait Execute {
    fn execute(&self, pid: Pid) -> Result<TargetStat, &'static str>;
}

pub trait Help {
    fn help() -> ();
}

macro_rules! define_help {
    ($implementee:ident, $help_msg:literal) => {
        impl Help for $implementee {
            fn help() -> () {
                println!($help_msg)
            }
        }
    };
}

#[derive(Debug)]
pub struct Step;

impl Execute for Step {
    fn execute(&self, pid: Pid) -> Result<TargetStat, &'static str> {
        ptrace::step(pid, None)
            .map(|_| TargetStat::Running)
            .map_err(|err_no| {
                eprintln!("ERRNO {}", err_no);
                "failed to PTRACE_SINGLESTEP"
            })
    }
}

define_help!(Step, "s/step = step through process");

#[derive(Debug)]
pub struct Continue;

impl Execute for Continue {
    fn execute(&self, pid: Pid) -> Result<TargetStat, &'static str> {
        ptrace::cont(pid, None)
            .map(|_| TargetStat::Running)
            .map_err(|err_no| {
                eprintln!("ERRNO {}", err_no);
                "failed to PTRACE_CONT"
            })
    }
}

define_help!(Continue, "c/continue = run through process");

#[derive(Debug)]
pub struct ViewRegisters;

impl Execute for ViewRegisters {
    fn execute(&self, pid: Pid) -> Result<TargetStat, &'static str> {
        match ptrace::getregs(pid) {
            Ok(regs) => {
                println!(
                    "%RIP: {:#0x}\n\
                    %RAX: {:#0x}\n%RBX {:#0x}\n%RCX: {:#0x}\n%RDX: {:#0x}\n\
                    %RBP: {:#0x}\n%RSP: {:#0x}\n%RSI: {:#0x}\n%RDI: {:#0x}",
                    regs.rip,
                    regs.rax,
                    regs.rbx,
                    regs.rcx,
                    regs.rdx,
                    regs.rbp,
                    regs.rsp,
                    regs.rsi,
                    regs.rdi
                );
                Ok(TargetStat::AwaitingCommand)
            }

            Err(err_no) => {
                eprintln!("ERRNO {}", err_no);
                Err("failed to PTRACE_GETREGS")
            }
        }
    }
}

define_help!(ViewRegisters, "reg/registers = view register contents");

#[derive(Debug)]
pub struct Quit;

impl Execute for Quit {
    fn execute(&self, pid: Pid) -> Result<TargetStat, &'static str> {
        ptrace::kill(pid)
            .map(|_| TargetStat::Killed)
            .map_err(|err_no| {
                eprintln!("ERRNO {}", err_no);
                "failed to terminate target process"
            })
    }
}

define_help!(Quit, "q/quit = quit debugger and kill process");

#[derive(Debug)]
pub struct HelpMe;

impl Execute for HelpMe {
    fn execute(&self, _pid: Pid) -> Result<TargetStat, &'static str> {
        dbg::print_help();
        Ok(TargetStat::AwaitingCommand)
    }
}

define_help!(HelpMe, "h/help = prints this help message");

#[derive(Debug)]
pub struct ReadWord {
    addr: *mut c_void,
}

impl Execute for ReadWord {
    fn execute(&self, pid: Pid) -> Result<TargetStat, &'static str> {
        ptrace::read(pid, self.addr)
            .map(|val| {
                println!("@ {:#0x}: {:#0x}", self.addr as usize, val);
                TargetStat::AwaitingCommand
            })
            .map_err(|err_no| {
                eprintln!("ERRNO {}", err_no);
                "failed to PTRACE_CONT"
            })
    }
}

define_help!(
    ReadWord,
    "r/read <hex address> = read word from process address space"
);

#[derive(Debug)]
pub struct WriteWord {
    addr: *mut c_void,
    val: *mut c_void,
}

impl Execute for WriteWord {
    fn execute(&self, pid: Pid) -> Result<TargetStat, &'static str> {
        unsafe {
            ptrace::write(pid, self.addr, self.val)
                .map(|_| TargetStat::AwaitingCommand)
                .map_err(|err_no| {
                    eprintln!("ERRNO {}", err_no);
                    "failed to PTRACE_CONT"
                })
        }
    }
}

define_help!(
    WriteWord,
    "w/write <hex address> <hex value> = write word to address in process space"
);

pub struct Breakpoint(BrkptRecord);

impl Execute for Breakpoint {
    fn execute(&self, _pid: Pid) -> Result<TargetStat, &'static str> {
        self.0.activate();
        Ok(TargetStat::AwaitingCommand)
    }
}

define_help!(
    Breakpoint,
    "b/breakpoint <file:line> = a standard breakpoint"
);

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

pub fn prompt_user_cmd(
    symbols: Option<Ref<'_, Dwarf<borrow::Cow<'_, [u8]>>>>,
    target_pid: Pid,
) -> Result<Box<dyn Execute>, &'static str> {
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
            if let Some(input_str) = args_iter.nth(0) {
                if let Ok(addr) = usize::from_str_radix(input_str, 16) {
                    Ok(Box::new(ReadWord {
                        addr: addr as *mut c_void,
                    }))
                } else {
                    Err("Failed to parse address: please supply hex value!")
                }
            } else {
                Err("Missing the address to read from!")
            }
        }

        // Commands with two operands
        "w" | "write" => {
            if let (Some(write_addr), Some(write_word)) = (args_iter.next(), args_iter.next()) {
                if let (Ok(parsed_addr), Ok(parsed_word)) = (
                    usize::from_str_radix(write_addr, 16),
                    usize::from_str_radix(write_word, 16),
                ) {
                    Ok(Box::new(WriteWord {
                        addr: parsed_addr as *mut c_void,
                        val: parsed_word as *mut c_void,
                    }))
                } else {
                    Err("Failed to parse args for writing word!")
                }
            } else {
                Err("Insufficient arguments for command!")
            }
        }
        "b" | "breakpoint" => {
            if let Some(symref) = symbols {
                let res = args_iter
                    .next()
                    .as_deref()
                    .ok_or("Insufficient arguments for command!")
                    .and_then(|arg| parse_file_and_lineno(arg))
                    .and_then(|(fname, lno)| {
                        src_line_to_addr(symref, fname, lno)
                            .map_err(|_| "Failed to resolve address!")
                    });

                match res {
                    Ok(addr) => Ok(Box::new(Breakpoint(BrkptRecord::new(
                        target_pid,
                        addr as *mut c_void,
                    )))),
                    Err(msg) => Err(msg),
                }
            } else {
                Err("Cannot resolve source lines without debug symbols!")
            }
        }

        _ => Err("Could not recognize command!"),
    }
}
