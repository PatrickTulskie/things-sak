//! Streamable HTTP transport for the MCP server.
//!
//! Security posture: binds to loopback by default. Binding to a LAN address
//! requires Bearer token auth (auto-generated if not provided). Tokens are
//! only accepted via the Authorization header, never in the URL.

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use anyhow::{Result, bail};
use axum::{
    Router,
    body::Body,
    extract::Request,
    http::{StatusCode, header::AUTHORIZATION},
    middleware::{self, Next},
    response::Response,
    routing::get,
};
use rand::distr::{Alphanumeric, Distribution};
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};
use subtle::ConstantTimeEq;

use crate::mcp;

#[derive(clap::Args)]
pub struct ServeArgs {
    /// Port to listen on
    #[arg(short, long, default_value_t = 32123)]
    pub port: u16,

    /// Address to bind. Use your LAN address (or 0.0.0.0) to allow other
    /// machines to connect; that requires token auth.
    #[arg(long, default_value = "127.0.0.1")]
    pub bind: IpAddr,

    /// Bearer token clients must send (or set THINGS_SAK_TOKEN).
    /// Auto-generated when binding beyond loopback and none is given.
    #[arg(long, env = "THINGS_SAK_TOKEN", hide_env_values = true)]
    pub token: Option<String>,

    /// Disable token auth (only allowed on loopback binds)
    #[arg(long, conflicts_with = "token")]
    pub no_token: bool,

    /// Host/authority allowed in the Host header (repeatable). Defaults to
    /// loopback hosts; when binding to a LAN address the bind address is
    /// allowed automatically.
    #[arg(long = "allowed-host")]
    pub allowed_hosts: Vec<String>,
}

fn generate_token() -> String {
    Alphanumeric
        .sample_iter(rand::rng())
        .take(32)
        .map(char::from)
        .collect()
}

fn bearer_ok(req: &Request, token: &str) -> bool {
    req.headers()
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|presented| presented.as_bytes().ct_eq(token.as_bytes()).into())
        .unwrap_or(false)
}

pub async fn run(args: ServeArgs) -> Result<()> {
    let loopback = args.bind.is_loopback();

    let token = if args.no_token {
        if !loopback {
            bail!("--no-token is only allowed when binding to loopback");
        }
        None
    } else {
        match args.token {
            Some(t) => Some((t, false)),
            None if loopback => None,
            None => Some((generate_token(), true)),
        }
    };

    let mut config = StreamableHttpServerConfig::default();
    if !args.allowed_hosts.is_empty() {
        config = config.with_allowed_hosts(args.allowed_hosts.clone());
    } else if !loopback {
        // Allow the addresses clients will actually put in the Host header.
        let mut hosts = vec![
            "localhost".to_string(),
            "127.0.0.1".to_string(),
            "::1".to_string(),
        ];
        if args.bind.is_unspecified() {
            // Can't know every interface address; rely on token auth instead.
            config = config.disable_allowed_hosts();
        } else {
            hosts.push(args.bind.to_string());
            config = config.with_allowed_hosts(hosts);
        }
    }

    let mcp_service = StreamableHttpService::new(
        || Ok(mcp::server_from_env()),
        Arc::new(LocalSessionManager::default()),
        config,
    );

    let mut mcp_router = Router::new().nest_service("/mcp", mcp_service);
    if let Some((token_value, _)) = &token {
        let expected = token_value.clone();
        mcp_router = mcp_router.layer(middleware::from_fn(move |req: Request, next: Next| {
            let expected = expected.clone();
            async move {
                if bearer_ok(&req, &expected) {
                    next.run(req).await
                } else {
                    Response::builder()
                        .status(StatusCode::UNAUTHORIZED)
                        .body(Body::from("Unauthorized"))
                        .unwrap()
                }
            }
        }));
    }

    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .merge(mcp_router);

    let addr = SocketAddr::new(args.bind, args.port);
    let listener = tokio::net::TcpListener::bind(addr).await?;

    print_banner(addr, token.as_ref());

    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;

    Ok(())
}

fn print_banner(addr: SocketAddr, token: Option<&(String, bool)>) {
    let host = if addr.ip().is_unspecified() {
        "<this-machine>".to_string()
    } else {
        addr.ip().to_string()
    };
    let url = format!("http://{host}:{}/mcp", addr.port());

    eprintln!("things-sak MCP server");
    eprintln!("  endpoint : {url}");
    match token {
        Some((t, true)) => {
            eprintln!("  token    : {t}  (generated for this run; pass --token to pin one)");
        }
        Some((_, false)) => {
            eprintln!("  token    : (set via --token / THINGS_SAK_TOKEN)");
        }
        None => {
            eprintln!("  token    : disabled (loopback only)");
        }
    }
    eprintln!();
    eprintln!("  MCP client config:");
    eprintln!("    {{");
    eprintln!("      \"mcpServers\": {{");
    eprintln!("        \"things\": {{");
    eprintln!("          \"type\": \"http\",");
    eprintln!(
        "          \"url\": \"{url}\"{}",
        if token.is_some() { "," } else { "" }
    );
    if let Some((t, generated)) = token {
        let shown = if *generated {
            t.as_str()
        } else {
            "<your token>"
        };
        eprintln!("          \"headers\": {{ \"Authorization\": \"Bearer {shown}\" }}");
    }
    eprintln!("        }}");
    eprintln!("      }}");
    eprintln!("    }}");
}
