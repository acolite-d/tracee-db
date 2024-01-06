use nix::{errno::Errno, libc, sys::ptrace, unistd::Pid};

use std::ffi::{c_uint, c_void};
use std::ptr;

#[derive(PartialEq, Eq, Debug)]
pub struct BrkptRecord {
    pub pid: Pid,
    pub pc_addr: *mut c_void,
    pub original_insn: i64,
}

impl BrkptRecord {
    pub fn new(pid: Pid, text_addr: *mut c_void) -> Self {
        let original_insn =
            ptrace::read(pid, text_addr).expect("Failed to read text region for breakpoint!");

        Self {
            pid,
            pc_addr: text_addr,
            original_insn,
        }
    }

    pub fn activate(&self) {
        let trap = ((self.original_insn & 0xFFFFFF00) | 0xCC) as *mut c_void;
        unsafe {
            ptrace::write(self.pid, self.pc_addr, trap).unwrap();
        }
    }

    pub fn recover_from_trap(&self) {
        println!("Recovering from trap!");
        unsafe {
            ptrace::write(self.pid, self.pc_addr, self.original_insn as *mut c_void)
                .expect("failed to write to .text section with PTRACE_POKEDATA");
        }
        let mut regs = ptrace::getregs(self.pid).expect("FATAL: Failed to send PTRACE_GETREGS");
        regs.rip -= 1;
        ptrace::setregs(self.pid, regs).expect("FATAL: Failed to send message PTRACE_SETREGS");
    }
}
