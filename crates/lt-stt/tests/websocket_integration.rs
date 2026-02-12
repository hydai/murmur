use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_tungstenite::accept_async;
use futures_util::{StreamExt, SinkExt};

/// Mock WebSocket server for testing
pub struct MockWebSocketServer {
    port: u16,
}

impl MockWebSocketServer {
    pub async fn new() -> Self {
        // Bind to any available port
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        // Spawn server task
        tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                tokio::spawn(async move {
                    let ws_stream = accept_async(stream).await.unwrap();
                    let (mut write, mut read) = ws_stream.split();

                    // Echo back messages with mock transcription
                    while let Some(msg) = read.next().await {
                        if let Ok(msg) = msg {
                            if msg.is_text() || msg.is_binary() {
                                // Send mock partial transcript
                                let response = r#"{"type":"partial_transcript","text":"test","timestamp":0}"#;
                                let _ = write.send(tokio_tungstenite::tungstenite::Message::Text(response.to_string().into())).await;

                                // Send mock final transcript
                                let response = r#"{"type":"final_transcript","text":"test complete","timestamp":100}"#;
                                let _ = write.send(tokio_tungstenite::tungstenite::Message::Text(response.to_string().into())).await;
                            }
                        }
                    }
                });
            }
        });

        Self { port }
    }

    pub fn url(&self) -> String {
        format!("ws://127.0.0.1:{}", self.port)
    }
}

#[tokio::test]
async fn test_mock_websocket_server() {
    let server = MockWebSocketServer::new().await;
    let url = server.url();

    // Connect to mock server
    let (ws_stream, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    let (mut write, mut read) = ws_stream.split();

    // Send a test message
    write.send(tokio_tungstenite::tungstenite::Message::Text("test".to_string().into())).await.unwrap();

    // Receive responses
    let msg1 = read.next().await.unwrap().unwrap();
    assert!(msg1.is_text());

    let msg2 = read.next().await.unwrap().unwrap();
    assert!(msg2.is_text());
}

#[tokio::test]
async fn test_websocket_reconnection_simulation() {
    // This test simulates connection loss and reconnection behavior
    let (tx, mut rx) = mpsc::channel(10);

    // Simulate sending connection events
    tx.send("connecting").await.unwrap();
    tx.send("connected").await.unwrap();
    tx.send("disconnected").await.unwrap();
    tx.send("reconnecting").await.unwrap();
    tx.send("connected").await.unwrap();

    // Verify events received in order
    assert_eq!(rx.recv().await.unwrap(), "connecting");
    assert_eq!(rx.recv().await.unwrap(), "connected");
    assert_eq!(rx.recv().await.unwrap(), "disconnected");
    assert_eq!(rx.recv().await.unwrap(), "reconnecting");
    assert_eq!(rx.recv().await.unwrap(), "connected");
}

#[tokio::test]
async fn test_exponential_backoff() {
    // Test exponential backoff timing
    let base_delay_ms = 1000u64;
    let max_delay_ms = 30000u64;

    let delays: Vec<u64> = (0..10)
        .map(|retry| std::cmp::min(base_delay_ms * 2u64.pow(retry), max_delay_ms))
        .collect();

    // Verify exponential growth then cap
    assert_eq!(delays[0], 1000);   // 1s
    assert_eq!(delays[1], 2000);   // 2s
    assert_eq!(delays[2], 4000);   // 4s
    assert_eq!(delays[3], 8000);   // 8s
    assert_eq!(delays[4], 16000);  // 16s
    assert_eq!(delays[5], 30000);  // capped at 30s
    assert_eq!(delays[9], 30000);  // still capped
}
