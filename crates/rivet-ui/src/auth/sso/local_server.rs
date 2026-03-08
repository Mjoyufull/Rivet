use std::net::TcpListener;
use tokio::sync::oneshot;
use url::Url;

/// Handle for awaiting the SSO redirect callback.
pub struct LocalServerHandle {
    /// The URL to redirect to (with port).
    pub redirect_url: Url,
    /// Receiver for the callback URL with login token.
    rx: oneshot::Receiver<Url>,
    /// Sender used to cancel the listener when the login flow is aborted.
    _shutdown_tx: oneshot::Sender<()>,
}

impl LocalServerHandle {
    /// Wait for the SSO callback to complete.
    pub async fn wait_for_callback(&mut self) -> Result<Url, LocalServerError> {
        (&mut self.rx)
            .await
            .map_err(|_| LocalServerError::Cancelled)
    }
}

/// Errors from the local SSO server.
#[derive(Debug)]
pub enum LocalServerError {
    BindFailed(std::io::Error),
    Cancelled,
}

impl std::fmt::Display for LocalServerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BindFailed(e) => write!(f, "failed to bind to local port: {}", e),
            Self::Cancelled => write!(f, "SSO login was cancelled"),
        }
    }
}

impl std::error::Error for LocalServerError {}

/// Spawn a local HTTP server to capture SSO redirect.
///
/// The server binds to 127.0.0.1 on a random available port and waits
/// for the homeserver to redirect the browser back with a login token.
pub fn spawn_local_server() -> Result<LocalServerHandle, LocalServerError> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(LocalServerError::BindFailed)?;
    let port = listener
        .local_addr()
        .map_err(LocalServerError::BindFailed)?
        .port();
    drop(listener);

    let redirect_url = Url::parse(&format!("http://127.0.0.1:{}/", port)).expect("valid URL");

    let (tx, rx) = oneshot::channel();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    tokio::spawn(run_server(port, tx, shutdown_rx));

    Ok(LocalServerHandle {
        redirect_url,
        rx,
        _shutdown_tx: shutdown_tx,
    })
}

async fn run_server(port: u16, tx: oneshot::Sender<Url>, shutdown_rx: oneshot::Receiver<()>) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    let listener = match TcpListener::bind(format!("127.0.0.1:{}", port)).await {
        Ok(listener) => listener,
        Err(e) => {
            tracing::error!("Failed to bind local server: {}", e);
            return;
        }
    };

    tracing::info!("SSO callback server listening on port {}", port);

    let mut shutdown_rx = shutdown_rx;
    let accept_result = tokio::select! {
        result = listener.accept() => Some(result),
        _ = &mut shutdown_rx => {
            tracing::info!("SSO callback server cancelled on port {}", port);
            None
        }
    };

    if let Some(Ok((mut socket, addr))) = accept_result {
        tracing::info!("SSO callback received from {}", addr);
        let mut buf = vec![0u8; 8192];
        if let Ok(n) = socket.read(&mut buf).await {
            let request = String::from_utf8_lossy(&buf[..n]);
            tracing::debug!("SSO callback request:\n{}", request);

            if let Some(first_line) = request.lines().next() {
                tracing::debug!("Request line: {}", first_line);
                if let Some(path) = first_line.strip_prefix("GET ") {
                    if let Some(path) = path.split_whitespace().next() {
                        tracing::info!("SSO callback path: {}", path);
                        let full_url = format!("http://127.0.0.1:{}{}", port, path);
                        tracing::info!("Full callback URL: {}", full_url);
                        if let Ok(callback_url) = Url::parse(&full_url) {
                            for (key, value) in callback_url.query_pairs() {
                                tracing::info!("SSO param: {} = {}", key, value);
                            }
                            let _ = tx.send(callback_url);
                        } else {
                            tracing::error!("Failed to parse callback URL: {}", full_url);
                        }
                    }
                }
            }

            let response = concat!(
                "HTTP/1.1 200 OK\r\n",
                "Content-Type: text/html\r\n",
                "Connection: close\r\n",
                "\r\n",
                "<!DOCTYPE html><html><head><meta charset=\"utf-8\">",
                "<title>Login Complete</title>",
                "<style>body{font-family:system-ui;display:flex;justify-content:center;",
                "align-items:center;height:100vh;margin:0;background:#1e1e2e;color:#cdd6f4;}",
                ".container{text-align:center;}.check{font-size:48px;color:#a6e3a1;}</style>",
                "</head><body><div class=\"container\">",
                "<div class=\"check\">✓</div>",
                "<h1>Login Successful</h1>",
                "<p>You can close this window and return to Rivet.</p>",
                "</div></body></html>"
            );

            let _ = socket.write_all(response.as_bytes()).await;
        }
    }
}
