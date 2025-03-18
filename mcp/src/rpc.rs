use crate::{Error, Service};
use axum::{
    Json,
    extract::State,
    response::sse::{Event, Sse},
};
use futures::stream::{self, Stream};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    convert::Infallible,
    hash::{Hash, Hasher},
    sync::{Arc, Mutex},
};
use tokio::sync::broadcast;
use tokio::sync::oneshot;
use tokio_stream::StreamExt;
use tracing::{debug, error, info, warn};

pub struct McpImpl<S> {
    tx: broadcast::Sender<ServerResponse>,
    cancel: Mutex<HashMap<RequestId, oneshot::Sender<()>>>,
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
#[allow(clippy::large_enum_variant)]
pub enum ServerResponse {
    Response(mcp_schema::JSONRPCResponse<mcp_schema::ServerResult>),
    Notification(mcp_schema::ServerNotification),
    Error(mcp_schema::JSONRPCError),
    None,
}

#[derive(Debug, Clone)]
struct RequestId(mcp_schema::RequestId);

impl PartialEq for RequestId {
    fn eq(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (mcp_schema::RequestId::String(x), mcp_schema::RequestId::String(y)) => x == y,
            (mcp_schema::RequestId::Number(x), mcp_schema::RequestId::Number(y)) => x == y,
            _ => false,
        }
    }
}

impl Eq for RequestId {}

impl Hash for RequestId {
    fn hash<H>(&self, state: &mut H)
    where
        H: Hasher,
    {
        match &self.0 {
            mcp_schema::RequestId::String(x) => x.hash(state),
            mcp_schema::RequestId::Number(x) => x.hash(state),
        }
    }
}

impl<S: Service + Send + Sync> McpImpl<S> {
    #[must_use]
    #[allow(dead_code)]
    pub fn new(mut service: S) -> Self {
        let (tx, _) = broadcast::channel(100);

        let tx_clone = tx.clone();
        service.set_notification_handler(Box::new(move |notification| {
            if let Err(e) = tx_clone.send(ServerResponse::Notification(notification)) {
                warn!("Failed to broadcast response: {}", e);
            } else {
                debug!("Successfully broadcast response");
            }
        }));
        Self {
            tx,
            cancel: Mutex::new(HashMap::new()),
            service,
        }
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
                let id = RequestId(request_id(&request).clone());
                let (cancel_sender, cancel_receiver) = oneshot::channel();
                state
                    .cancel
                    .lock()
                    .unwrap()
                    .insert(id.clone(), cancel_sender);
                let response = tokio::select! {
                    response = handle_request(&state.service, request) => response,
                    _ = cancel_receiver => return Json(ServerResponse::None)
                };
                state.cancel.lock().unwrap().remove(&id);

                let response = match response {
                    Ok(response) => ServerResponse::Response(response),
                    Err(error) => ServerResponse::Error(mcp_schema::JSONRPCError {
                        json_rpc: mcp_schema::JSONRPC_VERSION.to_string(),
                        id: id.0,
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
            ClientMessage::Notification(notification) => {
                if let mcp_schema::ClientNotification::Cancelled { params, .. } = notification {
                    let id = RequestId(params.request_id);
                    if let Some(reason) = params.reason {
                        warn!("client cancelled client request {id:?} with reason: {reason}");
                    } else {
                        warn!("client cancelled client request {id:?} with no reason provided");
                    }
                    let sender = state.cancel.lock().unwrap().remove(&id);
                    if let Some(sender) = sender {
                        if sender.send(()).is_err() {
                            error!("cancellation receiver was dropped");
                        }
                    } else {
                        // This may occur if the request finished on the server side and the
                        // result has not yet been sent to the client. Therefore, this isn't treated as an error.
                        warn!(
                            "client attempted to cancel client request {id:?} but it is not in progress - this is likely harmless"
                        );
                    }
                }
                Json(ServerResponse::None)
            }
        }
    }
}

const fn request_id(request: &mcp_schema::ClientRequest) -> &mcp_schema::RequestId {
    match request {
        mcp_schema::ClientRequest::Initialize { id, .. }
        | mcp_schema::ClientRequest::Ping { id, .. }
        | mcp_schema::ClientRequest::ListResources { id, .. }
        | mcp_schema::ClientRequest::ListResourceTemplates { id, .. }
        | mcp_schema::ClientRequest::ReadResource { id, .. }
        | mcp_schema::ClientRequest::Subscribe { id, .. }
        | mcp_schema::ClientRequest::Unsubscribe { id, .. }
        | mcp_schema::ClientRequest::ListPrompts { id, .. }
        | mcp_schema::ClientRequest::GetPrompt { id, .. }
        | mcp_schema::ClientRequest::ListTools { id, .. }
        | mcp_schema::ClientRequest::CallTool { id, .. }
        | mcp_schema::ClientRequest::SetLevel { id, .. }
        | mcp_schema::ClientRequest::Complete { id, .. } => id,
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

#[expect(clippy::too_many_lines)]
async fn handle_request(
    service: &(impl Service + Send + Sync),
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
