mod traceedb;
use traceedb::dbg::TraceeDb;

use std::env;
use std::fs;

fn main() {
    println!("NEXTDB DEBUGGER\nType \"help\" for command list!");

    let mut args = env::args().skip(1).take(2);
    let elf_buf: Vec<u8>;

    let mut builder = TraceeDb::builder();

    if let Some(prog) = args.next() {
        builder = builder.program(prog)
    }

    if let Some(dbg_file) = args.next() {
        elf_buf = fs::read(dbg_file).expect("failed to load file for symbols!");
        builder = builder.dwarf_symbols(elf_buf.as_slice());
    }

    builder.build().run();
}
