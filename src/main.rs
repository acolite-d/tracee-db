mod traceedb;
use traceedb::function::TraceeDb;

use std::env;
use std::ffi::CString;

fn main() {
    println!("NEXTDB DEBUGGER\nType \"help\" for command list!");

    let mut args = env::args()
        .skip(1)
        .map(|arg| CString::new(arg).expect("Improper input!"));

    TraceeDb::builder()
        .target_exec(args.next())
        .initial_break(args.next())
        .build()
        .run();
}
