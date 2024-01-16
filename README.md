# tracee-db

A x86_64 GNU/Linux debugger written in Rust. Associated with a series of blog posts found here.

1. [Forays into Systems Programming: “Who Watches the Watchmen?”–Writing An GNU/Linux x86_64 Debugger with Rust and the Nix Crate](https://find.thedoorman.xyz/?p=305)
2. [Writing a GNU/Linux x86_64 Debugger in Rust (Part 2): A Rust-ic Refactoring](https://find.thedoorman.xyz/?p=312)
3. [Writing a GNU/Linux x86_64 Debugger in Rust (Part 3): Implementing Breakpoints with DWARF, the Gimli Crate, and Traps](https://find.thedoorman.xyz/?p=314)

# Building and Running Debugger

Building is done via `cargo`. Run it and specify an executable with the run subcommand, offering the arguments after double dash.

```sh
cargo build
cargo run -- myexecutable
```

```
TRACEEDB DEBUGGER
Type "help" for command list!
Spawned child process 162194
Entering debugging loop...
Running traceable target program "test"
> help
List of Commands:
s/step = step through process
c/continue = run through process
reg/registers = view register contents
r/read <hex address> = read word from process address space
w/write <hex address> <hex value> = write word to address in process space
b/breakpoint <file:line> = a standard breakpoint
q/quit = quit debugger and kill process
h/help = prints this help message
```
