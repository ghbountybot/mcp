use crate::{Error, Service};
use axum::{
    Json,
    extract::State,
    response::sse::{Event, Sse},
};
use futures::stream::{self, Stream};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, convert::Infallible, sync::Arc};
use tokio::sync::broadcast;
use tokio_stream::StreamExt;
use tracing::{debug, info, warn};

pub struct McpImpl<S> {
    tx: broadcast::Sender<ServerResponse>,
    service: S,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum ClientMessage {
    Request(mcp_schema::ClientRequest),
    Notification(mcp_schema::ClientNotification),
}

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum ServerResponse {
    Response(mcp_schema::JSONRPCResponse<mcp_schema::ServerResult>),
    Error(mcp_schema::JSONRPCError),
    None,
}

impl<S: Service> McpImpl<S> {
    #[must_use]
    #[allow(dead_code)]
    pub fn new(service: S) -> Self {
        let (tx, _) = broadcast::channel(100);
        Self { tx, service }
    }

    #[allow(clippy::unused_async)]
    pub async fn sse_handler(
        State(state): State<Arc<Self>>,
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
        State(state): State<Arc<Self>>,
        Json(message): Json<ClientMessage>,
    ) -> Json<ServerResponse> {
        debug!("Message details: {:?}", message);

        match message {
            ClientMessage::Request(request) => {
                let id = request_id(&request).clone();
                let response = handle_request(&state.service, request).await;

                let response = match response {
                    Ok(response) => ServerResponse::Response(response),
                    Err(error) => ServerResponse::Error(mcp_schema::JSONRPCError {
                        json_rpc: mcp_schema::JSONRPC_VERSION.to_string(),
                        id,
                        error: mcp_schema::RPCErrorDetail {
                            code: error.code,
                            message: error.message,
                            data: None,
                        },
                    }),
                };

                if let Err(e) = state.tx.send(response.clone()) {
                    warn!("Failed to broadcast response: {}", e);
                } else {
                    debug!("Successfully broadcast response");
                }

                Json(response)
            }
            ClientMessage::Notification(_) => Json(ServerResponse::None),
        }
    }
}

fn request_id(request: &mcp_schema::ClientRequest) -> &mcp_schema::RequestId {
    match request {
        mcp_schema::ClientRequest::Initialize { id, .. } => id,
        mcp_schema::ClientRequest::Ping { id, .. } => id,
        mcp_schema::ClientRequest::ListResources { id, .. } => id,
        mcp_schema::ClientRequest::ListResourceTemplates { id, .. } => id,
        mcp_schema::ClientRequest::ReadResource { id, .. } => id,
        mcp_schema::ClientRequest::Subscribe { id, .. } => id,
        mcp_schema::ClientRequest::Unsubscribe { id, .. } => id,
        mcp_schema::ClientRequest::ListPrompts { id, .. } => id,
        mcp_schema::ClientRequest::GetPrompt { id, .. } => id,
        mcp_schema::ClientRequest::ListTools { id, .. } => id,
        mcp_schema::ClientRequest::CallTool { id, .. } => id,
        mcp_schema::ClientRequest::SetLevel { id, .. } => id,
        mcp_schema::ClientRequest::Complete { id, .. } => id,
    }
}

fn checked_version(json_rpc: String) -> Result<String, Error> {
    let expected = mcp_schema::JSONRPC_VERSION;
    if json_rpc == expected {
        Ok(json_rpc)
    } else {
        Err(Error {
            message: format!(
                "Client is using JSON RPC version {json_rpc}, but server only supports version {expected}"
            ),
            code: 400,
        })
    }
}

async fn handle_request(
    service: &impl Service,
    request: mcp_schema::ClientRequest,
) -> Result<mcp_schema::JSONRPCResponse<mcp_schema::ServerResult>, Error> {
    let response = match request {
        mcp_schema::ClientRequest::Initialize {
            json_rpc,
            id,
            params,
        } => mcp_schema::JSONRPCResponse {
            json_rpc: checked_version(json_rpc)?,
            id,
            result: service
                .init(params)
                .await
                .map(mcp_schema::ServerResult::Initialize)?,
        },
        mcp_schema::ClientRequest::Ping {
            json_rpc,
            id,
            params,
        } => mcp_schema::JSONRPCResponse {
            json_rpc: checked_version(json_rpc)?,
            id,
            result: service
                .ping(params)
                .await
                .map(mcp_schema::ServerResult::Empty)?,
        },
        mcp_schema::ClientRequest::ListResources {
            json_rpc,
            id,
            params,
        } => mcp_schema::JSONRPCResponse {
            json_rpc: checked_version(json_rpc)?,
            id,
            result: service
                .list_resources(params)
                .await
                .map(mcp_schema::ServerResult::ListResources)?,
        },
        mcp_schema::ClientRequest::ListResourceTemplates {
            json_rpc,
            id,
            params,
        } => mcp_schema::JSONRPCResponse {
            json_rpc: checked_version(json_rpc)?,
            id,
            result: service
                .list_resource_templates(params)
                .await
                .map(mcp_schema::ServerResult::ListResourceTemplates)?,
        },
        mcp_schema::ClientRequest::ReadResource {
            json_rpc,
            id,
            params,
        } => mcp_schema::JSONRPCResponse {
            json_rpc: checked_version(json_rpc)?,
            id,
            result: service
                .read_resource(params)
                .await
                .map(mcp_schema::ServerResult::ReadResource)?,
        },
        mcp_schema::ClientRequest::Subscribe {
            json_rpc,
            id,
            params,
        } => mcp_schema::JSONRPCResponse {
            json_rpc: checked_version(json_rpc)?,
            id,
            result: service
                .subscribe(params)
                .await
                .map(mcp_schema::ServerResult::Empty)?,
        },
        mcp_schema::ClientRequest::Unsubscribe {
            json_rpc,
            id,
            params,
        } => mcp_schema::JSONRPCResponse {
            json_rpc: checked_version(json_rpc)?,
            id,
            result: service
                .unsubscribe(params)
                .await
                .map(mcp_schema::ServerResult::Empty)?,
        },
        mcp_schema::ClientRequest::ListPrompts {
            json_rpc,
            id,
            params,
        } => mcp_schema::JSONRPCResponse {
            json_rpc: checked_version(json_rpc)?,
            id,
            result: service
                .list_prompts(params)
                .await
                .map(mcp_schema::ServerResult::ListPrompts)?,
        },
        mcp_schema::ClientRequest::GetPrompt {
            json_rpc,
            id,
            params,
        } => mcp_schema::JSONRPCResponse {
            json_rpc: checked_version(json_rpc)?,
            id,
            result: service
                .get_prompt(params)
                .await
                .map(mcp_schema::ServerResult::GetPrompt)?,
        },
        mcp_schema::ClientRequest::ListTools {
            json_rpc,
            id,
            params,
        } => mcp_schema::JSONRPCResponse {
            json_rpc: checked_version(json_rpc)?,
            id,
            result: service
                .list_tools(params)
                .await
                .map(mcp_schema::ServerResult::ListTools)?,
        },
        mcp_schema::ClientRequest::CallTool {
            json_rpc,
            id,
            params,
        } => mcp_schema::JSONRPCResponse {
            json_rpc: checked_version(json_rpc)?,
            id,
            result: service
                .call_tool(params)
                .await
                .map(mcp_schema::ServerResult::CallTool)?,
        },
        mcp_schema::ClientRequest::SetLevel {
            json_rpc,
            id,
            params,
        } => mcp_schema::JSONRPCResponse {
            json_rpc: checked_version(json_rpc)?,
            id,
            result: service
                .set_level(params)
                .await
                .map(mcp_schema::ServerResult::Empty)?,
        },
        mcp_schema::ClientRequest::Complete { json_rpc, id, .. } => mcp_schema::JSONRPCResponse {
            json_rpc: checked_version(json_rpc)?,
            id,
            result: mcp_schema::ServerResult::Empty(mcp_schema::EmptyResult {
                meta: None,
                extra: HashMap::new(),
            }),
        },
    };

    Ok(response)
}
