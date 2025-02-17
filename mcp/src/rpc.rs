use axum::{
    Json,
    extract::State,
    response::sse::{Event, Sse},
};
use futures::stream::{self, Stream};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::HashMap, convert::Infallible, future::Future, pin::Pin, sync::Arc};
use tokio::sync::broadcast;
use tokio_stream::StreamExt;
use tracing::{debug, info, warn};

type JsonRpcFuture = Pin<Box<dyn Future<Output = JsonRpcResponse> + Send>>;
type HandlerFn = Box<dyn Fn(Value) -> JsonRpcFuture + Send + Sync>;

#[derive(Clone)]
pub struct McpImpl {
    tx: Arc<broadcast::Sender<JsonRpcMessage>>,
    handlers: Arc<HashMap<String, HandlerFn>>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<i32>,
    pub method: String,
    pub params: Option<Value>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct JsonRpcNotification {
    pub jsonrpc: String,
    pub method: String,
    pub params: Option<Value>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum JsonRpcMessage {
    Request(JsonRpcRequest),
    Response(JsonRpcResponse),
    Notification(JsonRpcNotification),
}

impl Default for McpImpl {
    fn default() -> Self {
        let (tx, _) = broadcast::channel(100);
        Self {
            tx: Arc::new(tx),
            handlers: Arc::new(HashMap::new()),
        }
    }
}

impl McpImpl {
    #[must_use]
    #[allow(dead_code)]
    pub fn new(handlers: HashMap<String, HandlerFn>) -> Self {
        let (tx, _) = broadcast::channel(100);
        Self {
            tx: Arc::new(tx),
            handlers: Arc::new(handlers),
        }
    }

    #[allow(clippy::unused_async)]
    pub async fn sse_handler(
        State(state): State<Self>,
    ) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
        info!("New SSE connection established");
        let rx = state.tx.subscribe();

        // Send initial endpoint event as required by MCP spec
        let endpoint_url = "/api/message";
        debug!("Sending initial endpoint URL: {}", endpoint_url);

        let initial =
            stream::once(async move { Ok(Event::default().event("endpoint").data(endpoint_url)) });

        let stream = stream::unfold(rx, |mut rx| async move {
            match rx.recv().await {
                Ok(msg) => {
                    debug!("Broadcasting message: {:?}", msg);
                    let event = Event::default().event("message").json_data(msg).ok()?;
                    Some((Ok(event), rx))
                }
                Err(e) => {
                    warn!("Error receiving message: {}", e);
                    None
                }
            }
        });

        Sse::new(initial.chain(stream))
    }

    pub async fn message_handler(
        State(state): State<Self>,
        Json(request): Json<JsonRpcRequest>,
    ) -> Json<JsonRpcResponse> {
        info!("Received message request - method: {}", request.method);
        debug!("Message request details: {:?}", request);

        let response = match request.method.as_str() {
            "initialize" => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: Some(serde_json::json!({
                    "serverInfo": {
                        "name": "mcp-weather",
                        "version": "0.1.0"
                    },
                    "protocolVersion": "2024-11-05",
                    "capabilities": {
                        "tools": {
                            "listChanged": false
                        }
                    }
                })),
                error: None,
            },
            "tools/list" => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: Some(serde_json::json!({
                    "tools": [
                        {
                            "name": "get_alerts",
                            "description": "Get weather alerts for a US state",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "state": {
                                        "type": "string",
                                        "description": "Two-letter US state code (e.g. CA, NY)"
                                    }
                                },
                                "required": ["state"]
                            }
                        },
                        {
                            "name": "get_forecast",
                            "description": "Get weather forecast for a location",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "latitude": {
                                        "type": "number",
                                        "description": "Latitude of the location"
                                    },
                                    "longitude": {
                                        "type": "number",
                                        "description": "Longitude of the location"
                                    }
                                },
                                "required": ["latitude", "longitude"]
                            }
                        }
                    ]
                })),
                error: None,
            },
            method => {
                if let Some(handler) = state.handlers.get(method) {
                    let params = request.params.unwrap_or(Value::Null);
                    handler(params).await
                } else {
                    JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id: request.id,
                        result: None,
                        error: Some(JsonRpcError {
                            code: -32601,
                            message: format!("Method not found: {method}"),
                            data: None,
                        }),
                    }
                }
            }
        };

        if let Err(e) = state.tx.send(JsonRpcMessage::Response(response.clone())) {
            warn!("Failed to broadcast response: {}", e);
        } else {
            debug!("Successfully broadcast response");
        }

        Json(response)
    }
}

#[allow(dead_code)]
pub struct McpHandler {
    pub(crate) name: String,
    pub(crate) handler: HandlerFn,
}

impl McpHandler {
    #[allow(dead_code)]
    pub fn new<F>(name: &str, f: F) -> Self
    where
        F: Fn(Value) -> JsonRpcFuture + Send + Sync + 'static,
    {
        Self {
            name: name.to_string(),
            handler: Box::new(f),
        }
    }
}
