use std::path::PathBuf;

use clap::Parser;
use tokio::net::TcpListener;

#[derive(Debug, Parser)]
#[command(name = "server", about = "Runs the HAM HTTP server on 127.0.0.1:3000")]
struct ServerCli;

#[tokio::main]
async fn main() {
    let _ = domain::domain_ready();

    let _cli = ServerCli::parse();

    let db_path = std::env::var("HAM_DB_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("assets.db"));
    let app = server::app::build_app(db_path).expect("failed to build app");

    let listener = TcpListener::bind("127.0.0.1:3000")
        .await
        .expect("failed to bind 127.0.0.1:3000");
    axum::serve(listener, app).await.expect("server failed");
}

#[cfg(test)]
mod tests {
    use super::ServerCli;
    use clap::{error::ErrorKind, Parser};

    #[test]
    fn parse_accepts_no_flags() {
        let parsed = ServerCli::try_parse_from(["server"]);
        assert!(parsed.is_ok());
    }

    #[test]
    fn parse_rejects_unknown_flags() {
        let err = ServerCli::try_parse_from(["server", "--port", "3000"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::UnknownArgument);
    }

    #[test]
    fn parse_supports_help_flag() {
        let err = ServerCli::try_parse_from(["server", "--help"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::DisplayHelp);
    }
}
