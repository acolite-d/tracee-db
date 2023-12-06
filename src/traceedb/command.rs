use super::function::{self, print_help};
use nix::{
    sys::ptrace,
    sys::signal::{kill, Signal},
    sys::wait::{waitpid, WaitStatus},
    unistd::{execv, Pid},
};

use std::ffi::c_void;
use std::io::{stdin, stdout, Write};


pub trait Execute {
    fn execute(self, pid: Pid) -> ();
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
    fn execute(self, pid: Pid) -> () {
        ptrace::step(pid, None).expect("single step ptrace message failed!");
    }
}

define_help!(Step, "s/step = step through process");

#[derive(Debug)]
pub struct Continue;

impl Execute for Continue {
    fn execute(self, pid: Pid) -> () {
        ptrace::cont(pid, None).expect("PTRACE_CONT message failed to send!");
    }
}

define_help!(Continue, "c/continue = run through process");

#[derive(Debug)]
pub struct ViewRegisters;

impl Execute for ViewRegisters {
    fn execute(self, pid: Pid) -> () {
        function::print_register_status(pid);
    }
}

define_help!(ViewRegisters, "reg/registers = view register contents");

#[derive(Debug)]
pub struct Quit;

impl Execute for Quit {
    fn execute(self, pid: Pid) -> () {
        ptrace::kill(pid).expect("Failed to kill process!");
    }
}

define_help!(Quit, "q/quit = quit debugger and kill process");

#[derive(Debug)]
pub struct HelpMe;

impl Execute for HelpMe {
    fn execute(self, _pid: Pid) -> () {
        function::print_help();
    }
}

define_help!(HelpMe, "h/help = prints this help message");

#[derive(Debug)]
pub struct ReadWord {
    addr: *mut c_void,
}

impl Execute for ReadWord {
    fn execute(self, pid: Pid) -> () {
        function::read_word(pid, self.addr);
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
    fn execute(self, pid: Pid) -> () {
        function::write_word(pid, self.addr, self.val);
    }
}

define_help!(
    WriteWord,
    "write <hex address> <hex value> = write word to address in process space"
);

// fn debugger_loop() -> () {
//     let current_cmd: 

//     'process_loop: loop {
//         let wait_status = waitpid(target_pid, None);

//         'input_loop: loop {



//             match get_cmd() {
//                 Ok(cmd) => {
//                     cmd.execute(pid);
//                     // if ctx.process_wait then continue 'process_loop
//                     // else continue 'user_loop (fall through to main loop)
//                 },

//                 Err(err_msg) => eprintln!("Command not recognized: {}", err_msg)
//             }
//         }
//     }
// }

// fn get_cmd<C>() -> Box<dyn C>
//     where C: Execute + Help
// {
//     Box::new(Quit)
// }

// pub fn get_cmd() -> Box<dyn Cmd> {
//     print!("> ");
//     stdout().flush().unwrap();

//     let mut user_input = String::new();

//     while let Err(_) = stdin().read_line(&mut user_input) {
//         eprintln!("Err: Failed to read user input, please enter a proper command!");
//         user_input.clear();
//     }

//     let mut term_iter = user_input.split_whitespace();

//     let (command, mut args_iter) = (term_iter.nth(0).unwrap(), term_iter);

//     match command {
//         // Commands with no operands
//         "reg" | "registers" => Box::new(ViewRegisters),
//         "s" | "step" => Box::new(Step),
//         "c" | "continue" => Box::new(Continue),
//         "q" | "quit" => Box::new(Quit),
//         "h" | "help" => Box::new(HelpMe),

//         // Commands with a single operand
//         "r" | "read" => {
//             if let Some(read_addr) = args_iter.nth(0) {
//                 if let Ok(parsed_addr) = usize::from_str_radix(read_addr, 16) {
//                     Box::new(ReadWord {addr: parsed_addr})
//                 } else {
//                     eprintln!("Failed to parse args!");
//                     Command::Unknown
//                 }
//             } else {
//                 eprintln!("Insufficient args!");
//                 Command::Unknown
//             }
//         }

//         // Commands with two operands
//         "w" | "write" => {
//             if let (Some(write_addr), Some(write_word)) = (args_iter.next(), args_iter.next()) {
//                 if let (Ok(parsed_addr), Ok(parsed_word)) = (
//                     usize::from_str_radix(write_addr, 16),
//                     usize::from_str_radix(write_word, 16),
//                 ) {
//                     Command::Write(parsed_addr as *mut c_void, parsed_word as *mut c_void)
//                 } else {
//                     eprintln!("Failed to parse args!");
//                     Command::Unknown
//                 }
//             } else {
//                 eprintln!("Insufficient args!");
//                 Command::Unknown
//             }
//         }

//         _ => Command::Unknown,
//     }
// }

pub enum Command {
    Help,
    Step,
    Continue,
    ViewRegisters,
    Quit,
    Unknown,
    Read(*mut c_void),
    Write(*mut c_void, *mut c_void),
}

pub fn accept_user_input() -> Command {
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
        "reg" | "registers" => Command::ViewRegisters,
        "s" | "step" => Command::Step,
        "c" | "continue" => Command::Continue,
        "q" | "quit" => Command::Quit,
        "h" | "help" => Command::Help,

        // Commands with a single operand
        "r" | "read" => {
            if let Some(read_addr) = args_iter.nth(0) {
                if let Ok(parsed_addr) = usize::from_str_radix(read_addr, 16) {
                    Command::Read(parsed_addr as *mut c_void)
                } else {
                    eprintln!("Failed to parse args!");
                    Command::Unknown
                }
            } else {
                eprintln!("Insufficient args!");
                Command::Unknown
            }
        }

        // Commands with two operands
        "w" | "write" => {
            if let (Some(write_addr), Some(write_word)) = (args_iter.next(), args_iter.next()) {
                if let (Ok(parsed_addr), Ok(parsed_word)) = (
                    usize::from_str_radix(write_addr, 16),
                    usize::from_str_radix(write_word, 16),
                ) {
                    Command::Write(parsed_addr as *mut c_void, parsed_word as *mut c_void)
                } else {
                    eprintln!("Failed to parse args!");
                    Command::Unknown
                }
            } else {
                eprintln!("Insufficient args!");
                Command::Unknown
            }
        }

        _ => Command::Unknown,
    }
}
