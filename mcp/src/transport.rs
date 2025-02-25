//! # Transport Layer for MCP
//! 
//! This module provides transport abstractions for the Model Context Protocol (MCP).
//! Transports handle the low-level communication between clients and servers,
//! abstracting away the details of how messages are sent and received.
//!
//! ## Available Transports
//!
//! - [`StdioTransport`]: Uses standard input/output for communication
//! - [`SseTransport`]: Uses Server-Sent Events (SSE) for communication
//!
//! ## Custom Transports
//!
//! You can implement the [`Transport`] trait to create custom transport mechanisms
//! for specific use cases, such as WebSockets, TCP sockets, or other communication
//! channels.

use async_trait::async_trait;
use eyre::{Result};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::broadcast;
use axum::response::sse::{Event, Sse};
use futures::stream::{self};
use std::time::Duration;

use crate::message::McpMessage;

/// Transport trait for abstracting I/O mechanisms for MCP communication
///
/// This trait defines the interface for sending and receiving MCP messages
/// over different transport mechanisms. Implementations of this trait handle
/// the details of serializing and deserializing messages, as well as the
/// underlying I/O operations.
#[async_trait]
pub trait Transport: Send + Sync {
    /// Read a message from the transport
    ///
    /// Returns `None` if the transport has been closed or reached end-of-file.
    /// Returns an error if there was a problem reading from the transport or
    /// deserializing the message.
    async fn read_message(&mut self) -> Result<Option<McpMessage>>;
    
    /// Write a message to the transport
    ///
    /// Serializes the message and writes it to the transport.
    /// Returns an error if there was a problem serializing the message or
    /// writing to the transport.
    async fn write_message(&mut self, message: McpMessage) -> Result<()>;
}

/// Standard I/O transport implementation
///
/// This transport uses standard input and output streams for communication.
/// It's useful for command-line applications and for integrating with other
/// processes via pipes.
pub struct StdioTransport {
    /// Buffered reader for standard input
    reader: BufReader<tokio::io::Stdin>,
    
    /// Writer for standard output
    writer: tokio::io::Stdout,
}

impl StdioTransport {
    /// Create a new StdioTransport
    ///
    /// Initializes the transport with buffered standard input and output streams.
    #[must_use]
    pub fn new() -> Self {
        StdioTransport {
            reader: BufReader::new(tokio::io::stdin()),
            writer: tokio::io::stdout(),
        }
    }
}

#[async_trait]
impl Transport for StdioTransport {
    async fn read_message(&mut self) -> Result<Option<McpMessage>> {
        let mut line = String::new();
        let bytes_read = self.reader.read_line(&mut line).await?;
        if bytes_read == 0 {
            return Ok(None); // EOF
        }
        let message = serde_json::from_str(&line)?;
        Ok(Some(message))
    }

    async fn write_message(&mut self, message: McpMessage) -> Result<()> {
        let json = serde_json::to_string(&message)?;
        self.writer.write_all(json.as_bytes()).await?;
        self.writer.write_all(b"\n").await?; // Newline delimiter
        self.writer.flush().await?;
        Ok(())
    }
}

/// Server-Sent Events (SSE) transport implementation
///
/// This transport uses Server-Sent Events for communication, which is a
/// unidirectional communication channel from server to client over HTTP.
/// It's useful for web applications that need to receive real-time updates
/// from the server.
///
/// The `SseTransport` uses a broadcast channel internally to allow multiple
/// clients to receive the same messages.
pub struct SseTransport {
    /// Sender for broadcasting messages to all connected clients
    tx: broadcast::Sender<McpMessage>,
    
    /// Receiver for receiving messages from the broadcast channel
    rx: broadcast::Receiver<McpMessage>,
}

impl Clone for SseTransport {
    /// Clone the transport, creating a new receiver but sharing the same sender
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
            rx: self.tx.subscribe(),
        }
    }
}

impl SseTransport {
    /// Create a new SseTransport with the specified channel capacity
    ///
    /// # Arguments
    ///
    /// * `capacity` - The maximum number of messages that can be buffered in the channel
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        let (tx, rx) = broadcast::channel(capacity);
        Self { tx, rx }
    }
    
    /// Create a new SseTransport with an existing sender
    ///
    /// This is useful when you want to share a sender between multiple transports.
    ///
    /// # Arguments
    ///
    /// * `tx` - The broadcast sender to use
    #[must_use]
    pub fn new_with_sender(tx: broadcast::Sender<McpMessage>) -> Self {
        Self {
            rx: tx.subscribe(),
            tx,
        }
    }
    
    /// Get a clone of the broadcast sender
    ///
    /// This can be used to create additional transports that share the same sender.
    #[must_use]
    pub fn sender(&self) -> broadcast::Sender<McpMessage> {
        self.tx.clone()
    }
    
    /// Create an SSE handler for use with Axum
    ///
    /// This method returns a handler that can be used with Axum to serve
    /// Server-Sent Events to clients.
    ///
    /// # Returns
    ///
    /// An Axum response that streams SSE events to the client
    #[must_use]
    pub fn sse_handler(self) -> impl axum::response::IntoResponse {
        let initial = stream::once(async { Ok::<_, std::convert::Infallible>(Event::default().data("connected")) });
        let rx = self.tx.subscribe();
        
        let stream = stream::unfold(rx, |mut rx| async move {
            match rx.recv().await {
                Ok(msg) => {
                    let event = match Event::default().event("message").json_data(msg) {
                        Ok(event) => event,
                        Err(_) => return None, // Skip messages that can't be serialized
                    };
                    Some((Ok(event), rx))
                }
                Err(_) => None,
            }
        });
        
        Sse::new(futures::StreamExt::chain(initial, stream))
            .keep_alive(
                axum::response::sse::KeepAlive::new()
                    .interval(Duration::from_secs(15))
                    .text("keep-alive")
            )
    }
}

#[async_trait]
impl Transport for SseTransport {
    async fn read_message(&mut self) -> Result<Option<McpMessage>> {
        match self.rx.recv().await {
            Ok(message) => Ok(Some(message)),
            Err(_) => Ok(None), // Channel closed or other error
        }
    }

    async fn write_message(&mut self, message: McpMessage) -> Result<()> {
        // Ignore send errors (no receivers) as this is a broadcast
        let _ = self.tx.send(message);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::{McpMessage, InitializeParams};
    
    #[tokio::test]
    async fn test_sse_transport() {
        let transport = SseTransport::new(10);
        let tx = transport.sender();
        let mut rx = tx.subscribe();
        
        let message = McpMessage::Initialize(InitializeParams {
            protocol_version: "2024-11-05".to_string(),
        });
        
        tx.send(message.clone()).unwrap();
        
        let received = rx.recv().await.unwrap();
        assert!(matches!(received, McpMessage::Initialize(_)));
    }
} 