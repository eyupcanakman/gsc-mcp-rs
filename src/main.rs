use rmcp::ServiceExt;
use std::env;
use std::sync::Arc;

mod auth;
mod client;
mod output;
mod tools;
mod types;

#[derive(Debug)]
enum Command {
    Stdio,
    Auth,
    Help,
    Version,
}

fn parse_args() -> Command {
    let args: Vec<String> = env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("auth") => Command::Auth,
        Some("--help" | "-h" | "help") => Command::Help,
        Some("--version" | "-V") => Command::Version,
        Some("stdio") | None => Command::Stdio,
        Some(unknown) => {
            eprintln!(
                "[gsc-mcp-rs] Unknown command '{unknown}'. Run 'gsc-mcp-rs --help' for usage."
            );
            std::process::exit(1);
        }
    }
}

fn print_help() {
    println!(
        "gsc-mcp-rs {} -- Google Search Console MCP Server\n\n\
         Usage: gsc-mcp-rs <command>\n\n\
         Commands:\n\
         \x20 stdio              Start MCP server on stdio (default)\n\
         \x20 auth               Run interactive OAuth authentication flow\n\
         \x20 help, --help, -h   Show this help message\n\
         \x20 --version, -V      Show version",
        env!("CARGO_PKG_VERSION")
    );
}

#[tokio::main]
async fn main() {
    let cmd = parse_args();

    match cmd {
        Command::Stdio => {
            eprintln!("[gsc-mcp-rs] Starting stdio transport");
            let auth = auth::detect_auth().await;
            let client = Arc::new(client::GscClient::new(Arc::new(auth)));
            let server = tools::GscServer::new(client);
            let transport = rmcp::transport::io::stdio();
            match server.serve(transport).await {
                Ok(service) => {
                    eprintln!("[gsc-mcp-rs] Server running on stdio");
                    if let Err(e) = service.waiting().await {
                        eprintln!("[gsc-mcp-rs] Server stopped: {e}");
                    }
                }
                Err(e) => {
                    eprintln!("[gsc-mcp-rs] Failed to start MCP server: {e}");
                    std::process::exit(1);
                }
            }
        }
        Command::Version => {
            println!("gsc-mcp-rs {}", env!("CARGO_PKG_VERSION"));
        }
        Command::Auth => {
            if !auth::oauth::is_interactive() {
                eprintln!(
                    "[gsc-mcp-rs] Error: 'auth' command requires an interactive terminal.\n\
                     Run this command directly in a terminal, not as an MCP subprocess."
                );
                std::process::exit(1);
            }

            let provider = match auth::oauth::OAuthProvider::new() {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("[gsc-mcp-rs] {e}");
                    std::process::exit(1);
                }
            };

            if let Err(e) = provider.run_interactive_flow().await {
                eprintln!("[gsc-mcp-rs] Auth failed: {e}");
                std::process::exit(1);
            }
        }
        Command::Help => {
            print_help();
        }
    }
}
