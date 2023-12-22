use crate::traceedb::breakpoint::BrkptRecord;
use nix::{sys::ptrace, unistd::Pid};

use std::ffi::c_void;

pub enum TargetStat {
    AwaitingCommand,
    Running,
    Killed,
    BreakpointAdded(BrkptRecord)
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
                    %RAX: {:#0x}\n%RBX: {:#0x}\n%RCX: {:#0x}\n%RDX: {:#0x}\n\
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
        println!("List of Commands:");
        Step::help();
        Continue::help();
        ViewRegisters::help();
        ReadWord::help();
        WriteWord::help();
        Breakpoint::help();
        Quit::help();
        HelpMe::help();
        Ok(TargetStat::AwaitingCommand)
    }
}

define_help!(HelpMe, "h/help = prints this help message");

#[derive(Debug)]
pub struct ReadWord {
    pub addr: *mut c_void,
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
    pub addr: *mut c_void,
    pub val: *mut c_void,
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

pub struct Breakpoint(pub u64);

impl Execute for Breakpoint {
    fn execute(&self, pid: Pid) -> Result<TargetStat, &'static str> {
        Ok(TargetStat::BreakpointAdded(BrkptRecord::new(pid, self.0 as *mut c_void)))
    }
}

define_help!(
    Breakpoint,
    "b/breakpoint <file:line> = a standard breakpoint"
);


