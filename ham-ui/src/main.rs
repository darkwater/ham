use std::process::{ExitCode, Termination as _};

mod gui;

#[tokio::main]
async fn main() -> ExitCode {
    gui::main().report()
}
