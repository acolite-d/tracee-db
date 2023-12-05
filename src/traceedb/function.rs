use super::command::*;
use nix::{
    sys::ptrace,
    sys::signal::{kill, Signal},
    sys::wait::{waitpid, WaitStatus},
    unistd::{execv, Pid},
};

use std::ffi::{c_void, CStr};
use std::io::Write;
use std::io::{stdin, stdout};

pub fn print_register_status(target_pid: Pid) {
    let regs = ptrace::getregs(target_pid).expect("Failed to get register status using ptrace!");

    println!(
        "%RIP: {:#0x}\n\
        %RAX: {:#0x}\n%RBX {:#0x}\n%RCX: {:#0x}\n%RDX: {:#0x}\n\
        %RBP: {:#0x}\n%RSP: {:#0x}\n%RSI: {:#0x}\n%RDI: {:#0x}",
        regs.rip, regs.rax, regs.rbx, regs.rcx, regs.rdx, regs.rbp, regs.rsp, regs.rsi, regs.rdi
    );
}

pub fn read_word(target_pid: Pid, addr: *mut c_void) {
    let res = ptrace::read(target_pid, addr).expect("Failed to send PTRACE_PEEK message!");
    println!("@{:#0x}: {:#0x}", addr as usize, res);
}

pub fn write_word(target_pid: Pid, addr: *mut c_void, word: *mut c_void) {
    unsafe {
        ptrace::write(target_pid, addr, word).expect("Failed to send PTRACE_POKE message!");
    }
}

pub fn print_help() {

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

                    Command::Help => print_help(),

                    Command::Read(addr) => read_word(target_pid, addr),

                    Command::Write(addr, word) => write_word(target_pid, addr, word),

                    Command::ViewRegisters => print_register_status(target_pid),

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
