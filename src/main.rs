use nix::{
    sys::ptrace,
    sys::signal::{kill, Signal},
    sys::wait::{waitpid, WaitStatus},
    unistd::{execv, fork, ForkResult, Pid},
};

use std::ffi::{c_void, CStr, CString};
use std::io::{stdin, stdout};
use std::{env, io::Write};

enum Command {
    Step,
    Continue,
    ViewRegisters,
    Read(*mut c_void),
    Write(*mut c_void, *mut c_void),
    Quit,
    Unknown,
}

fn main() {
    println!(
        "NEXTDB DEBUGGER\nCommands: s = step, reg = registers, c = continue running, q = quit"
    );

    // if env::args().len() != 2 {
    //     panic!("Please supply a single argument, the target program!");
    // }

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

fn run_get_pid_dialogue() -> Pid {
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

fn run_target(prog_name: &CStr) {
    println!("Running traceable target program {:?}", prog_name);

    ptrace::traceme().expect("Ptrace failed, cannot debug!");
    let _ = execv(prog_name, &[] as &[&CStr]).expect("Failed to spawn process");
}

fn run_debugger(target_pid: Pid) {
    println!("Entering debugging loop...");

    'outer: loop {
        let wait_status = waitpid(target_pid, None);

        loop {
            match wait_status {
                Ok(WaitStatus::Stopped(_, Signal::SIGTRAP))
                | Ok(WaitStatus::Stopped(_, Signal::SIGSTOP)) => match accept_user_input() {
                    Command::Quit => {
                        ptrace::kill(target_pid).expect("Failed to kill process!");
                        break 'outer;
                    }

                    Command::Read(addr) => {
                        read_word_memory_region(target_pid, addr);
                    }

                    Command::Write(addr, word) => {
                        write_word(target_pid, addr, word);
                    }

                    Command::ViewRegisters => {
                        print_register_status(target_pid);
                    }

                    Command::Step => {
                        ptrace::step(target_pid, None).expect("single step ptrace message failed!");
                        continue 'outer;
                    }

                    Command::Continue => {
                        ptrace::cont(target_pid, None)
                            .expect("PTRACE_CONT message failed to send!");
                        break 'outer;
                    }

                    Command::Unknown => {
                        eprintln!("Err: Unknown command, please input an available command!")
                    }
                },

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

fn accept_user_input() -> Command {
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
        "s" | "step"        => Command::Step,
        "c" | "continue"    => Command::Continue,
        "q" | "quit"        => Command::Quit,

        //Commands with a single operand
        "r" | "read" => {
            if let Some(read_addr) = args_iter.nth(0) {
                if let Ok(parsed_addr) = usize::from_str_radix(read_addr, 16) {
                    Command::Read(parsed_addr as *mut c_void)
                } else {
                    println!("Failed to parse to usize!");
                    Command::Unknown
                }
            } else {
                println!("Failed to get operand!");
                Command::Unknown
            }
        },

        "w" | "write" => {
            if let (Some(write_addr), Some(write_word)) = (args_iter.nth(0), args_iter.nth(0)) {
                if let (Ok(parsed_addr), Ok(parsed_word)) 
                    = (usize::from_str_radix(write_addr, 16), usize::from_str_radix(write_word, 16)) {
                    Command::Write(parsed_addr as *mut c_void, parsed_word as *mut c_void)
                } else {
                    println!("Failed to parse write!");
                    Command::Unknown
                }
            } else {
                println!("Insufficient args!");
                Command::Unknown
            }
        }

        _ => Command::Unknown,
    }
}

fn print_register_status(target_pid: Pid) {
    let regs =
        ptrace::getregs(target_pid).expect("Failed to get register status using ptrace!");

    println!(
        "%RIP: {:#0x}\n\
        %RAX: {:#0x}\n%RBX {:#0x}\n%RCX: {:#0x}\n%RDX: {:#0x}\n\
        %RBP: {:#0x}\n%RSP: {:#0x}\n%RSI: {:#0x}\n%RDI: {:#0x}",
        regs.rip, regs.rax, regs.rbx, regs.rcx, regs.rdx,
        regs.rbp, regs.rsp, regs.rsi, regs.rdi
    );
}

fn read_word_memory_region(target_pid: Pid, addr: *mut c_void) {
    let res = ptrace::read(target_pid, addr).expect("Failed to send PTRACE_PEEK message!");
    println!("@{:#0x}: {:#0x}", addr as usize, res);
}

fn write_word(target_pid: Pid, addr: *mut c_void, word: *mut c_void) {
    unsafe { ptrace::write(target_pid, addr, word).expect("Failed to send PTRACE_POKE message!"); }
}