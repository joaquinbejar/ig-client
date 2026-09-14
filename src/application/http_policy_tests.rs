//! Audited local-only transport tests. This module uses explicit fake config;
//! select `application::http::single_attempt_tests::` without other lib tests.

use super::{HttpClient, RequestPolicy, RetryConfig, make_http_request, transport_builder};
use crate::application::config::{Config, Credentials, RateLimiterConfig, RestApiConfig};
use crate::application::rate_limiter::RateLimitClass;
use crate::application::rate_limiter::RateLimiter;
use crate::error::AppError;
use reqwest::Method;
use serde_json::{Value, json};
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;
use tokio::task::{JoinHandle, JoinSet};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

type TestResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;
const IO_TIMEOUT: Duration = Duration::from_secs(5);

#[test]
fn test_single_attempt_classifier_uses_only_canonical_path() {
    for path in [
        "/custom?redirect=positions/otc",
        "/custom#prices/FAKE",
        "http://example.invalid/custom?redirect=positions/otc",
        "http://example.invalid/custom#workingorders/otc",
        "http://prices.example.invalid/custom?redirect=prices/FAKE",
    ] {
        for method in [Method::GET, Method::POST] {
            assert_eq!(
                super::classify_endpoint(&method, path),
                RateLimitClass::NonTrading
            );
        }
    }
    assert_eq!(
        super::classify_endpoint(&Method::POST, "http://example.invalid/positions/x/../otc"),
        RateLimitClass::Trading
    );
    assert_eq!(
        super::classify_endpoint(
            &Method::GET,
            "http://example.invalid/prices/FAKE?redirect=positions/otc"
        ),
        RateLimitClass::Historical
    );
}

#[derive(Clone, Copy)]
enum ResponseMode {
    DropAfterRequest,
    TruncatedSuccess,
    RefuseHttp2Stream,
}

/// Listens until explicit shutdown, so a replay on another connection remains
/// observable. Both the accept task and all connection tasks are joined.
struct WireServer {
    address: SocketAddr,
    requests: Arc<AtomicUsize>,
    shutdown: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<io::Result<()>>>,
}

impl WireServer {
    async fn start(mode: ResponseMode) -> io::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let requests = Arc::new(AtomicUsize::new(0));
        let recorded = requests.clone();
        let (shutdown, mut stopped) = oneshot::channel();
        let task = tokio::spawn(async move {
            let mut connections = JoinSet::new();
            let result = loop {
                tokio::select! {
                    _ = &mut stopped => break Ok(()),
                    accepted = listener.accept() => {
                        let (stream, _) = match accepted { Ok(pair) => pair, Err(error) => break Err(error) };
                        let recorded = recorded.clone();
                        connections.spawn(async move {
                            tokio::time::timeout(IO_TIMEOUT, async move {
                                match mode {
                                    ResponseMode::RefuseHttp2Stream => refuse_http2(stream, recorded).await,
                                    _ => receive_http1(stream, recorded, mode).await,
                                }
                            }).await.map_err(io::Error::other)?
                        });
                    }
                    completed = connections.join_next(), if !connections.is_empty() => {
                        match completed {
                            Some(Ok(Ok(()))) => {}
                            Some(Ok(Err(error))) => break Err(error),
                            Some(Err(error)) => break Err(io::Error::other(error)),
                            None => {}
                        }
                    }
                }
            };
            connections.shutdown().await;
            result
        });
        Ok(Self {
            address,
            requests,
            shutdown: Some(shutdown),
            task: Some(task),
        })
    }

    fn url(&self) -> String {
        format!("http://{}/positions/otc", self.address)
    }

    async fn finish(mut self) -> io::Result<usize> {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(task) = self.task.take() {
            task.await.map_err(io::Error::other)??;
        }
        Ok(self.requests.load(Ordering::SeqCst))
    }
}

impl Drop for WireServer {
    fn drop(&mut self) {
        // Also cancel on assertion failure. Dropping the task's JoinSet cancels
        // every connection worker; the normal path explicitly awaits them.
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

async fn receive_http1(
    mut stream: TcpStream,
    requests: Arc<AtomicUsize>,
    mode: ResponseMode,
) -> io::Result<()> {
    let mut header = Vec::new();
    while !header.ends_with(b"\r\n\r\n") {
        header.push(stream.read_u8().await?);
        if header.len() > 16_384 {
            return Err(io::Error::other("fixture header too large"));
        }
    }
    let header = String::from_utf8(header).map_err(io::Error::other)?;
    let content_length = header
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then_some(value.trim())
        })
        .ok_or_else(|| io::Error::other("fixture expected JSON content length"))?
        .parse::<usize>()
        .map_err(io::Error::other)?;
    if content_length > 16_384 {
        return Err(io::Error::other("fixture body too large"));
    }
    let mut body = vec![0; content_length];
    stream.read_exact(&mut body).await?;
    let _: Value = serde_json::from_slice(&body).map_err(io::Error::other)?;
    // Only count an actual complete HTTP request, not a TCP connection attempt.
    requests.fetch_add(1, Ordering::SeqCst);
    if matches!(mode, ResponseMode::TruncatedSuccess) {
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 80\r\nConnection: close\r\n\r\n{")
            .await?;
    }
    stream.shutdown().await
}

async fn refuse_http2(mut stream: TcpStream, requests: Arc<AtomicUsize>) -> io::Result<()> {
    let mut preface = [0; 24];
    stream.read_exact(&mut preface).await?;
    if &preface != b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n" {
        return Err(io::Error::other("fixture expected HTTP/2 preface"));
    }
    // Empty server SETTINGS. No HPACK parsing is needed to count request streams.
    stream.write_all(&[0, 0, 0, 4, 0, 0, 0, 0, 0]).await?;
    loop {
        let mut frame = [0; 9];
        match stream.read_exact(&mut frame).await {
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(()),
            Err(error) => return Err(error),
        }
        let [a, b, c, kind, flags, s1, s2, s3, s4] = frame;
        let size = (usize::from(a) << 16) | (usize::from(b) << 8) | usize::from(c);
        if size > 65_536 {
            return Err(io::Error::other("fixture frame too large"));
        }
        let mut payload = vec![0; size];
        stream.read_exact(&mut payload).await?;
        if kind == 4 && flags & 1 == 0 {
            stream.write_all(&[0, 0, 0, 4, 1, 0, 0, 0, 0]).await?;
        } else if kind == 1 {
            requests.fetch_add(1, Ordering::SeqCst);
            // RST_STREAM with REFUSED_STREAM (7) is precisely a protocol NACK
            // that reqwest's default transport silently replays.
            stream
                .write_all(&[0, 0, 4, 3, 0, s1, s2, s3, s4, 0, 0, 0, 7])
                .await?;
        }
    }
}

#[tokio::test]
async fn test_single_attempt_transport_disables_actual_http2_replays() -> TestResult {
    for (policy, expected) in [
        (RequestPolicy::SingleAttempt, 1),
        (RequestPolicy::Standard, 3),
    ] {
        let server = WireServer::start(ResponseMode::RefuseHttp2Stream).await?;
        // This is the production transport builder. Prior knowledge only lets
        // the local fixture use HTTP/2 without certificates or a TLS dependency.
        let client = transport_builder(policy)
            .http2_prior_knowledge()
            .timeout(IO_TIMEOUT)
            .build()?;
        let limiter = RateLimiter::new(&RateLimiterConfig {
            max_requests: 1000,
            period_seconds: 1,
            burst_size: 1000,
        });
        let result = make_http_request(
            &client,
            &limiter,
            Method::POST,
            &server.url(),
            vec![],
            &Some(json!({"test": true})),
            RetryConfig::with_max_retries_and_delay(0, 0),
        )
        .await;
        let observed = server.finish().await?;
        assert!(matches!(result, Err(AppError::Network(_))));
        assert_eq!(observed, expected, "actual request streams for {policy:?}");
    }
    Ok(())
}

#[tokio::test]
async fn test_single_attempt_after_received_request_or_truncated_success() -> TestResult {
    for (index, mode) in [
        ResponseMode::DropAfterRequest,
        ResponseMode::TruncatedSuccess,
    ]
    .into_iter()
    .enumerate()
    {
        let login = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/session"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "clientId": "FAKE-CLIENT", "accountId": "FAKE-ACCOUNT", "timezoneOffset": 1,
                "lightstreamerEndpoint": "https://example.invalid",
                "oauthToken": { "access_token": "FAKE-ACCESS", "refresh_token": "FAKE-REFRESH",
                    "scope": "profile", "token_type": "Bearer", "expires_in": "3600" }
            })))
            .expect(1)
            .mount(&login)
            .await;
        let config = Config {
            rest_api: RestApiConfig {
                base_url: login.uri(),
                timeout: 5,
            },
            api_version: Some(3),
            ..Config::from_credentials(Credentials::new(
                "fake-user".into(),
                "fake-password".into(),
                format!("WIRE-TEST-{index}"),
                "FAKE-KEY".into(),
            ))
        };
        let client = HttpClient::new_lazy(config)?;
        let server = WireServer::start(mode).await?;
        let result = tokio::time::timeout(
            IO_TIMEOUT,
            client.request::<_, Value>(
                Method::POST,
                &server.url(),
                Some(json!({"test": true})),
                Some(2),
            ),
        )
        .await?;
        let observed = server.finish().await?;
        assert!(matches!(result, Err(AppError::Network(_))));
        assert_eq!(observed, 1, "a received mutation must never be resent");
        assert_eq!(
            login
                .received_requests()
                .await
                .expect("recording enabled")
                .len(),
            1
        );
    }
    Ok(())
}
