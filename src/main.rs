mod traceedb;
use traceedb::function::TraceeDb;

use std::env;
use std::ffi::CString;


// Todos for refactor:
// 1. Implement dynamic dispatch, either by stack pointer or heap
//    where Command trait interface compartmentalizes command functionality
//    down to just Creating command and execute() for Step, Quit, etc.
//
// 2. Make builders for constructing the debugger, as TraceeDb object.
//    build out the target for it giving Option<Pid>
//
// 3. Build out better main loop with Commands, waits
// 
// Other things, Parser pattern

fn main() {
    println!("NEXTDB DEBUGGER\nType \"help\" for command list!");

    let target_program = env::args().map(|arg| CString::new(arg).unwrap()).nth(1);

    TraceeDb::builder()
        .target_exec(target_program)
        .build()
        .run();
}