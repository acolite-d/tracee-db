mod traceedb;
use traceedb::function::TraceeDb;

use std::env;
use std::ffi::CString;

// Todos for refactor:
// 1. Implement dynamic dispatch, either by stack pointer or heap
//    where Command trait interface compartmentalizes command functionality
//    down to just Creating command and execute() for Step, Quit, etc. MOSTLY DONE
//
// 2. Make builders for constructing the debugger, as TraceeDb object.
//    build out the target for it giving Option<Pid> DONE
//
// 3. Build out better main loop with Commands, waits
//
// Other things, Parser pattern

fn main() {
    println!("NEXTDB DEBUGGER\nType \"help\" for command list!");

    let mut args = env::args().map(|arg| CString::new(arg).unwrap());

    TraceeDb::builder()
        .target_exec(args.next())
        .initial_break(args.next())
        .build()
        .run();
}
