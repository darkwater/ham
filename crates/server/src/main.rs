use std::path::PathBuf;

use tokio::net::TcpListener;

fn should_print_help(args: &[String]) -> bool {
    args.iter().any(|arg| arg == "--help" || arg == "-h")
}

#[tokio::main]
async fn main() {
    let _ = domain::domain_ready();

    let args: Vec<String> = std::env::args().skip(1).collect();
    if should_print_help(&args) {
        println!("Usage: server");
        println!("Runs the HAM HTTP server on 127.0.0.1:3000");
        println!("Environment:");
        println!("  HAM_DB_PATH   Path to SQLite database (default: assets.db)");
        return;
    }

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
    use super::should_print_help;

    #[test]
    fn detects_long_help_flag() {
        let args = vec!["--help".to_string()];
        assert!(should_print_help(&args));
    }

    #[test]
    fn detects_short_help_flag() {
        let args = vec!["-h".to_string()];
        assert!(should_print_help(&args));
    }

    #[test]
    fn ignores_non_help_flags() {
        let args = vec!["--port".to_string(), "3000".to_string()];
        assert!(!should_print_help(&args));
    }
}
