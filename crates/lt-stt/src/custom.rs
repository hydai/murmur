//! Configurable OpenAI-compatible speech recognition.
pub const DEFAULT_MODEL: &str = "whisper-1";
pub type CustomSttProvider = crate::http::HttpSttProvider;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::MAX_PENDING_TRANSCRIPTION_CHUNKS;
    use lt_core::stt::{AudioChunk, SttProvider, TranscriptionEvent};
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio::sync::Mutex;
    use tokio::sync::{oneshot, Notify};
    use tokio::time::{timeout, Duration};

    #[test]
    fn test_custom_provider_creation() {
        let provider =
            CustomSttProvider::new("http://localhost:8080/v1".to_string(), None, None, None);
        assert_eq!(provider.model, "whisper-1");
        assert!(provider.api_key.is_none());
        assert!(provider.language.is_none());
    }

    #[test]
    fn test_custom_provider_with_options() {
        let provider = CustomSttProvider::new(
            "http://localhost:8080/v1".to_string(),
            Some("my-key".to_string()),
            Some("large-v3".to_string()),
            Some("en".to_string()),
        );
        assert_eq!(provider.model, "large-v3");
        assert_eq!(provider.api_key.as_deref(), Some("my-key"));
        assert_eq!(provider.language.as_deref(), Some("en"));
    }

    #[test]
    fn test_empty_strings_become_none() {
        let provider = CustomSttProvider::new(
            "http://localhost:8080/v1".to_string(),
            Some("".to_string()),
            Some("".to_string()),
            Some("".to_string()),
        );
        assert_eq!(provider.model, "whisper-1");
        assert!(provider.api_key.is_none());
        assert!(provider.language.is_none());
    }

    #[tokio::test]
    async fn send_audio_does_not_block_while_transcription_request_is_in_flight() {
        let server = HangingTranscriptionServer::start().await;
        let mut provider = CustomSttProvider::new(server.base_url(), None, None, None);

        provider.start_session().await.unwrap();
        let _events = provider.subscribe_events().await;

        provider.send_audio(test_chunk(1)).await.unwrap();
        provider.send_audio(test_chunk(4001)).await.unwrap();
        server.wait_for_request().await;

        let mut blocked_at = None;
        for i in 0..33 {
            let send = provider.send_audio(empty_test_chunk(4010 + i));
            match timeout(Duration::from_millis(100), send).await {
                Ok(Ok(())) => {}
                Ok(Err(e)) => panic!("send_audio returned error: {}", e),
                Err(_) => {
                    blocked_at = Some(i);
                    break;
                }
            }
            tokio::task::yield_now().await;
        }

        server.release_response();
        provider.stop_session().await.unwrap();

        assert!(
            blocked_at.is_none(),
            "send_audio blocked at queued chunk {:?} while a transcription request was in flight",
            blocked_at
        );
    }

    #[tokio::test]
    async fn transcription_handoff_has_bounded_backlog_while_request_is_in_flight() {
        let server = CountingTranscriptionServer::start().await;
        let mut provider = CustomSttProvider::new(server.base_url(), None, None, None);

        provider.start_session().await.unwrap();
        let mut events = provider.subscribe_events().await;

        provider.send_audio(test_chunk(1)).await.unwrap();
        provider.send_audio(test_chunk(4001)).await.unwrap();
        server.wait_for_requests(1).await;

        let queued_flushes = MAX_PENDING_TRANSCRIPTION_CHUNKS + 3;
        let mut timestamp_ms = 8002;
        for _ in 0..queued_flushes {
            provider.send_audio(test_chunk(timestamp_ms)).await.unwrap();
            provider
                .send_audio(test_chunk(timestamp_ms + 4000))
                .await
                .unwrap();
            timestamp_ms += 4001;
        }

        let overload = timeout(Duration::from_millis(200), events.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(
            matches!(overload, TranscriptionEvent::Error { message } if message.contains("backlog"))
        );
        server.release_first_response();
        timeout(
            Duration::from_secs(2),
            server.wait_for_requests(1 + MAX_PENDING_TRANSCRIPTION_CHUNKS),
        )
        .await
        .unwrap();
        provider.stop_session().await.unwrap();

        assert_eq!(
            server.request_count(),
            1 + MAX_PENDING_TRANSCRIPTION_CHUNKS,
            "transcription requests should be capped while the endpoint is backlogged"
        );
    }

    #[tokio::test]
    async fn stop_session_returns_when_custom_transcription_endpoint_hangs() {
        let server = HangingTranscriptionServer::start().await;
        let mut provider = CustomSttProvider::new(server.base_url(), None, None, None);

        provider.start_session().await.unwrap();
        let mut events = provider.subscribe_events().await;

        provider.send_audio(test_chunk(1)).await.unwrap();
        provider.send_audio(test_chunk(4001)).await.unwrap();
        server.wait_for_request().await;

        timeout(Duration::from_secs(2), provider.stop_session())
            .await
            .expect("stop_session should not wait forever for a hung transcription request")
            .unwrap();

        let event = timeout(Duration::from_secs(2), events.recv())
            .await
            .expect("timeout should emit a transcription error event")
            .expect("event channel should remain open");
        assert!(
            matches!(event, TranscriptionEvent::Error { ref message } if message.contains("timed out")),
            "expected timeout error event, got {:?}",
            event
        );
    }

    #[tokio::test]
    async fn dropping_provider_aborts_nested_request_and_closes_events() {
        let server = HangingTranscriptionServer::start().await;
        let mut provider = CustomSttProvider::new(server.base_url(), None, None, None);
        provider.start_session().await.unwrap();
        let mut events = provider.subscribe_events().await;
        provider.send_audio(test_chunk(1)).await.unwrap();
        provider.send_audio(test_chunk(4001)).await.unwrap();
        server.wait_for_request().await;
        drop(provider);
        assert!(timeout(Duration::from_secs(1), events.recv())
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn cancelling_stop_aborts_request_and_closes_events() {
        let server = HangingTranscriptionServer::start().await;
        let mut provider = CustomSttProvider::new(server.base_url(), None, None, None);
        provider.start_session().await.unwrap();
        let mut events = provider.subscribe_events().await;
        provider.send_audio(test_chunk(1)).await.unwrap();
        provider.send_audio(test_chunk(4001)).await.unwrap();
        server.wait_for_request().await;
        assert!(timeout(Duration::from_millis(10), provider.stop_session())
            .await
            .is_err());
        drop(provider);
        assert!(timeout(Duration::from_secs(1), events.recv())
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn dropping_event_receiver_cancels_inflight_and_queued_uploads() {
        let server = CountingTranscriptionServer::start().await;
        let mut provider = CustomSttProvider::new(server.base_url(), None, None, None);
        provider.start_session().await.unwrap();
        let events = provider.subscribe_events().await;
        provider.send_audio(test_chunk(1)).await.unwrap();
        provider.send_audio(test_chunk(4001)).await.unwrap();
        server.wait_for_requests(1).await;
        for index in 0..MAX_PENDING_TRANSCRIPTION_CHUNKS {
            let timestamp = 8002 + index as u64 * 4001;
            provider.send_audio(test_chunk(timestamp)).await.unwrap();
            provider
                .send_audio(test_chunk(timestamp + 4000))
                .await
                .unwrap();
        }
        drop(events);
        // The first response is still withheld: completion proves shutdown
        // cancelled the in-flight request instead of draining queued uploads.
        timeout(Duration::from_millis(200), provider.stop_session())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(server.request_count(), 1);
        server.release_first_response();
        tokio::task::yield_now().await;
        assert_eq!(server.request_count(), 1);
    }

    #[tokio::test]
    async fn final_buffer_survives_a_full_transcription_queue() {
        let server = CountingTranscriptionServer::start().await;
        let mut provider = CustomSttProvider::new(server.base_url(), None, None, None);
        provider.start_session().await.unwrap();
        let _events = provider.subscribe_events().await;
        provider.send_audio(test_chunk(1)).await.unwrap();
        provider.send_audio(test_chunk(4001)).await.unwrap();
        server.wait_for_requests(1).await;
        let mut timestamp = 8002;
        for _ in 0..MAX_PENDING_TRANSCRIPTION_CHUNKS {
            provider.send_audio(test_chunk(timestamp)).await.unwrap();
            provider
                .send_audio(test_chunk(timestamp + 4000))
                .await
                .unwrap();
            timestamp += 4001;
        }
        provider.send_audio(test_chunk(timestamp)).await.unwrap();
        let stopping = tokio::spawn(async move {
            provider.stop_session().await.unwrap();
        });
        tokio::time::sleep(Duration::from_millis(10)).await;
        server.release_first_response();
        timeout(Duration::from_secs(2), stopping)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(server.request_count(), MAX_PENDING_TRANSCRIPTION_CHUNKS + 2);
    }

    fn test_chunk(timestamp_ms: u64) -> AudioChunk {
        AudioChunk {
            data: vec![0; 160],
            timestamp_ms,
        }
    }

    fn empty_test_chunk(timestamp_ms: u64) -> AudioChunk {
        AudioChunk {
            data: Vec::new(),
            timestamp_ms,
        }
    }

    struct CountingTranscriptionServer {
        base_url: String,
        first_response_release_tx: Mutex<Option<oneshot::Sender<()>>>,
        request_count: Arc<AtomicUsize>,
        request_notify: Arc<Notify>,
    }

    impl CountingTranscriptionServer {
        async fn start() -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();
            let (first_response_release_tx, first_response_release_rx) = oneshot::channel();
            let request_count = Arc::new(AtomicUsize::new(0));
            let request_notify = Arc::new(Notify::new());

            let task_request_count = request_count.clone();
            let task_request_notify = request_notify.clone();
            tokio::spawn(async move {
                let mut first_response_release_rx = Some(first_response_release_rx);

                loop {
                    let Ok((mut stream, _)) = listener.accept().await else {
                        break;
                    };

                    read_http_request(&mut stream).await;
                    let count = task_request_count.fetch_add(1, Ordering::SeqCst) + 1;
                    task_request_notify.notify_one();

                    if count == 1 {
                        if let Some(rx) = first_response_release_rx.take() {
                            let _ = rx.await;
                        }
                    }

                    write_transcription_response(&mut stream).await;
                }
            });

            Self {
                base_url: format!("http://{}/v1", addr),
                first_response_release_tx: Mutex::new(Some(first_response_release_tx)),
                request_count,
                request_notify,
            }
        }

        fn base_url(&self) -> String {
            self.base_url.clone()
        }

        async fn wait_for_requests(&self, expected: usize) {
            while self.request_count() < expected {
                self.request_notify.notified().await;
            }
        }

        fn request_count(&self) -> usize {
            self.request_count.load(Ordering::SeqCst)
        }

        fn release_first_response(&self) {
            if let Ok(mut guard) = self.first_response_release_tx.try_lock() {
                if let Some(tx) = guard.take() {
                    let _ = tx.send(());
                }
            }
        }
    }

    struct HangingTranscriptionServer {
        base_url: String,
        request_seen_rx: Mutex<Option<oneshot::Receiver<()>>>,
        release_tx: Mutex<Option<oneshot::Sender<()>>>,
    }

    impl HangingTranscriptionServer {
        async fn start() -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();
            let (request_seen_tx, request_seen_rx) = oneshot::channel();
            let (release_tx, release_rx) = oneshot::channel();

            tokio::spawn(async move {
                let (mut stream, _) = listener.accept().await.unwrap();
                read_http_request(&mut stream).await;
                let _ = request_seen_tx.send(());
                let _ = release_rx.await;

                write_transcription_response(&mut stream).await;
            });

            Self {
                base_url: format!("http://{}/v1", addr),
                request_seen_rx: Mutex::new(Some(request_seen_rx)),
                release_tx: Mutex::new(Some(release_tx)),
            }
        }

        fn base_url(&self) -> String {
            self.base_url.clone()
        }

        async fn wait_for_request(&self) {
            let rx = self.request_seen_rx.lock().await.take().unwrap();
            rx.await.unwrap();
        }

        fn release_response(&self) {
            if let Ok(mut guard) = self.release_tx.try_lock() {
                if let Some(tx) = guard.take() {
                    let _ = tx.send(());
                }
            }
        }
    }

    async fn write_transcription_response(stream: &mut tokio::net::TcpStream) {
        let body = r#"{"text":"ok"}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        let _ = stream.write_all(response.as_bytes()).await;
    }

    async fn read_http_request(stream: &mut tokio::net::TcpStream) {
        let mut buf = Vec::new();
        let mut header_end = None;
        let mut tmp = [0u8; 4096];

        while header_end.is_none() {
            let n = stream.read(&mut tmp).await.unwrap();
            assert!(n > 0, "connection closed before HTTP headers");
            buf.extend_from_slice(&tmp[..n]);
            header_end = buf.windows(4).position(|window| window == b"\r\n\r\n");
        }

        let header_end = header_end.unwrap() + 4;
        let headers = String::from_utf8_lossy(&buf[..header_end]);
        let content_length = headers
            .lines()
            .find_map(|line| line.strip_prefix("content-length:"))
            .or_else(|| {
                headers
                    .lines()
                    .find_map(|line| line.strip_prefix("Content-Length:"))
            })
            .and_then(|value| value.trim().parse::<usize>().ok())
            .unwrap_or(0);

        let mut body_read = buf.len().saturating_sub(header_end);
        while body_read < content_length {
            let n = stream.read(&mut tmp).await.unwrap();
            assert!(n > 0, "connection closed before HTTP body");
            body_read += n;
        }
    }
}
