use std::process::{ExitCode, Termination as _};

// mod db;
// mod gui;
mod iced;

#[tokio::main]
async fn main() -> ExitCode {
    iced::main().report()
}
