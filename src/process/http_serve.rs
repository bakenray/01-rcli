use std::{net::SocketAddr, path::PathBuf, sync::Arc};
use anyhow::Result;
use axum::{Router, extract::{Path, State}, http::StatusCode, routing::get};
use tracing::{info, warn};
use tower_http::services::ServeDir;

#[derive(Debug)]
struct HttpServeState {
    path: PathBuf,
}

pub async fn process_http_serve(path: PathBuf, port: u16) -> Result<()> {
    let addr = SocketAddr::from(([0,0,0,0], port));
    info!("Serving {:?} on prot {}", path, port);

    let state = HttpServeState { path: path.clone() };

    let dir_service = ServeDir::new(path)
        .append_index_html_on_directories(true)
        .precompressed_gzip()
        .precompressed_br()
        .precompressed_zstd()
        .precompressed_deflate();

    let router = Router::new()
        .route("/{*path}", get(file_handler))
        .nest_service("/tower", dir_service)
        .with_state(Arc::new(state));
    
    let listener = tokio::net::TcpListener::bind(addr).await?;

    axum::serve(listener, router).await?;
    Ok(())
}

async fn file_handler(
    State(state): State<Arc<HttpServeState>>, 
    Path(path): Path<String>,
) -> (StatusCode, String) {
    let p = std::path::Path::new(&state.path).join(path);
    if !p.exists() {
        (
            StatusCode::NOT_FOUND, 
            format!("File {} not found: ", p.display())
        )
    } else {
        match tokio::fs::read_to_string(p).await {
            Ok(content) => {
                info!("Read {} bytes", content.len());
                (StatusCode::OK, content)
            }
            Err(e) => {
                warn!("Error reading file: {:?}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
            }
        } 
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_file_handler() {
        let state = Arc::new(HttpServeState{path: PathBuf::from(".")});
        let (status, content) = file_handler(State(state), Path("Cargo.toml".to_string())).await;
        assert_eq!(status, StatusCode::OK);
        assert!(content.trim().starts_with("[package]"));
    }
}