use std::process::{ExitCode, Termination as _};

mod gui;

fn main() -> ExitCode {
    gui::main().report()
}
