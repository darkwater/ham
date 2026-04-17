use std::process::{ExitCode, Termination as _};

// mod db;
// mod gui;
// mod iced;
mod gui;

fn main() -> ExitCode {
    gui::main().report()
}
