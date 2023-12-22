use nix::{
    errno::Errno,
    libc,
    sys::ptrace,
    unistd::Pid,
};

use std::ptr;
use std::ffi::{c_void, c_uint};


#[derive(PartialEq, Eq, Debug)]
pub struct BrkptRecord {
    pub pid: Pid,
    pub pc_addr: *mut c_void,
    pub original_insn: i64,
}

impl BrkptRecord {
    pub fn new(pid: Pid, pc_addr: *mut c_void) -> Self {
        //let original_insn = ptrace::read(pid, pc_addr).unwrap();
        let original_insn = peek_text(pid, pc_addr, ptr::null_mut()).unwrap();

        Self {
            pid,
            pc_addr,
            original_insn
        }
    }

    pub fn activate(&self) {
        let trap = ((self.original_insn & 0xFFFFFF00) | 0xCC) as *mut c_void;
        unsafe {
            poke_text(self.pid, self.pc_addr, trap).unwrap();
        }
    }

    pub fn recover_from_trap(&self) {
        todo!()
    }
}

unsafe fn poke_text(pid: Pid, addr: *mut c_void, val: *mut c_void) -> Result<(), &'static str> {
    Errno::result(libc::ptrace(
        ptrace::Request::PTRACE_POKETEXT as c_uint,
        libc::pid_t::from(pid),
        addr,
        val,
    ))
    .map(|_| ())
    .map_err(|_| "Failed to send PTRACE_POKETEXT message!")
}

fn peek_text(pid: Pid, addr: *mut c_void, data: *mut c_void) -> Result<i64, &'static str> {
    let ret = unsafe {
        Errno::clear();
        libc::ptrace(
            ptrace::Request::PTRACE_PEEKTEXT as c_uint,
            libc::pid_t::from(pid),
            addr,
            data,
        )
    };
    match Errno::result(ret) {
        Ok(..) | Err(Errno::UnknownErrno) => Ok(ret),
        Err(..) => Err("Failed to send PTRACE_PEEKTEXT message!"),
    }
}
