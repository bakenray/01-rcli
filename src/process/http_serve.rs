use std::{io, net::SocketAddr, path::PathBuf, sync::Arc};
use anyhow::Result;
use axum::{
    Router,
    extract::{Path, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
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
) -> Response {
    let p = std::path::Path::new(&state.path).join(&path);
    if !p.exists() {
        return (
            StatusCode::NOT_FOUND, 
            format!("File {} not found: ", p.display())
        ).into_response();
    }

    // 请求的是目录：生成 HTML 目录索引
    if p.is_dir() {
        return match list_dir(&p, &path).await {
            Ok(html) => {
                info!("Listed dir {} -> {} bytes", p.display(), html.len());
                (
                    StatusCode::OK,
                    [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
                    html,
                ).into_response()
            }
            Err(e) => {
                warn!("Error reading dir {:?}: {:?}", p, e);
                (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
            }
        };
    }

    match tokio::fs::read_to_string(&p).await {
        Ok(content) => {
            info!("Read {} bytes", content.len());
            (StatusCode::OK, content).into_response()
        }
        Err(e) => {
            warn!("Error reading file: {:?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    } 
}

/// 只遍历目录的第一级内容，拼成一个简单的 HTML 目录索引页。
async fn list_dir(dir: &std::path::Path, request_path: &str) -> io::Result<String> {
    // tokio::fs::read_dir 返回的 ReadDir 实现了 Stream，用 next_entry 逐个异步取出条目；
    // 标准库的 std::fs::read_dir 则是同步迭代器，这里用它会让出异步运行时的线程。
    let mut read_dir = tokio::fs::read_dir(dir).await?;
    let mut entries: Vec<(String, bool)> = Vec::new();
    while let Some(entry) = read_dir.next_entry().await? {
        // file_type() 比 metadata() 更轻量，只需要判断是不是目录
        let is_dir = entry.file_type().await?.is_dir();
        entries.push((entry.file_name().to_string_lossy().into_owned(), is_dir));
    }

    // 子目录排在前面，同类之间按名称排序，保证每次输出顺序稳定
    entries.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    // 以请求路径为基准拼出绝对 URL，避免缺少结尾斜杠时相对链接指向错误位置
    let mut base = String::from("/");
    if !request_path.is_empty() {
        base.push_str(request_path.trim_end_matches('/'));
        base.push('/');
    }

    let mut html = String::from("<!DOCTYPE html>\n<html>\n<head>\n<meta charset=\"utf-8\">\n");
    html.push_str(&format!("<title>Index of {}</title>\n</head>\n<body>\n", escape_html(&base)));
    html.push_str(&format!("<h1>Index of {}</h1>\n<ul>\n", escape_html(&base)));

    for (name, is_dir) in &entries {
        let url = if *is_dir {
            format!("{}{}/", base, encode_url_segment(name))
        } else {
            format!("{}{}", base, encode_url_segment(name))
        };
        let text = if *is_dir { format!("{name}/") } else { name.clone() };
        html.push_str(&format!(
            "<li><a href=\"{}\">{}</a></li>\n",
            escape_html(&url),
            escape_html(&text)
        ));
    }

    html.push_str("</ul>\n</body>\n</html>\n");
    Ok(html)
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// 逐字节做最小百分号编码，保证空格、#、? 以及非 ASCII 字节不会破坏链接。
fn encode_url_segment(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn body_string(resp: Response) -> String {
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .expect("read body");
        String::from_utf8(bytes.to_vec()).expect("utf8 body")
    }

    #[tokio::test]
    async fn test_file_handler() {
        let state = Arc::new(HttpServeState{path: PathBuf::from(".")});
        let resp = file_handler(State(state), Path("Cargo.toml".to_string())).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let content = body_string(resp).await;
        assert!(content.trim().starts_with("[package]"));
    }

    #[tokio::test]
    async fn test_file_handler_not_found() {
        let state = Arc::new(HttpServeState{path: PathBuf::from(".")});
        let resp = file_handler(State(state), Path("no-such-file.txt".to_string())).await;
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_file_handler_dir_index() {
        let state = Arc::new(HttpServeState{path: PathBuf::from(".")});
        let resp = file_handler(State(state), Path("src".to_string())).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let content = body_string(resp).await;
        assert!(content.contains("Index of /src/"));
        assert!(content.contains("href=\"/src/main.rs\""));
        // 子目录应当带结尾斜杠
        assert!(content.contains("href=\"/src/cli/\""));
    }
}