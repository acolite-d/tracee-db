mod traceedb;
use object::Object;
use object::ObjectKind;
use traceedb::dbg::TraceeDbg;

use std::env;
use std::fs;

fn main() {
    println!("TRACEEDB DEBUGGER\nType \"help\" for command list!");

    let mut args = env::args().skip(1).take(2);
    let elf_buf: Vec<u8>;

    let mut builder = TraceeDbg::builder();

    if let Some(prog) = args.next() {
        elf_buf = fs::read(prog.as_str()).expect("Given program not found, exiting");
        let file = object::File::parse(&*elf_buf).expect("Failed to parse program as ELF, exiting");

        let is_et_dyn = match file.kind() {
            ObjectKind::Executable => false,
            ObjectKind::Dynamic => true,
            _ => panic!("Please provide an ELF executable of type ET_DYN or ET_EXEC!"),
        };

        builder = builder.program(prog)
            .is_position_independent(is_et_dyn)
            .dwarf_symbols(&elf_buf.as_slice());
    }

    builder.build().run();
}
