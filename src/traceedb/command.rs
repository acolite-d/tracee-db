use std::ffi::c_void;
use std::io::{Write, stdin, stdout};

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