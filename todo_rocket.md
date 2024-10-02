# To Do - Rocket

# Rocket + Juniper + GraphQL WebSocket
```rust
use std::sync::{Arc, LazyLock};

use futures::{future, SinkExt, StreamExt};
use juniper::ScalarValue;
use juniper_graphql_ws::{graphql_transport_ws, ArcSchema, ConnectionConfig};
use rocket::State;
use thiserror::Error;
use tracing::info;
use ws::Message;

use crate::{project::projects_schema, ProjectsContext, ProjectsSchema};

#[rocket::get("/subscription")]
pub async fn subscribe<'r>(
    ctx: &State<ProjectsContext>,
    ws: ws::WebSocket,
) -> ws::Channel<'r> {
    static SCHEMA: LazyLock<Arc<ProjectsSchema>> = LazyLock::new(|| Arc::new(projects_schema()));
    let config = ConnectionConfig::new((*ctx.inner()).clone());
    let connection = graphql_transport_ws::Connection::new(ArcSchema(SCHEMA.clone()), config);
    let (g_tx, mut g_rx) = connection.split();

    let channel = ws.channel(|ws| {
        Box::pin(async move {
            let (mut w_tx, w_rx) = ws.split();
            let input = w_rx
                .map(|r| r.map(WsMessage))
                .forward(g_tx.sink_map_err(|e| match e {}));

            let output = Box::pin(async move {
                while let Some(s_out) = g_rx.next().await {
                    info!("ws sending: {:?}", s_out);
                    match s_out {
                        graphql_transport_ws::Output::Message(m) => {
                            let s_out = ws::Message::text(serde_json::to_string(&m).unwrap());
                            w_tx.send(s_out).await.unwrap();
                        }

                        graphql_transport_ws::Output::Close { .. } => {
                            let s_out = ws::Message::Close(None);
                            w_tx.send(s_out).await.unwrap();
                            break;
                        }
                    }
                }
            });

            let _r = future::select(input, output).await;

            Ok(())
        })
    });

    channel
}

#[derive(Debug)]
struct WsMessage(ws::Message);
impl<S: ScalarValue> TryFrom<WsMessage> for graphql_transport_ws::Input<S> {
    type Error = WsMessageError;

    fn try_from(value: WsMessage) -> Result<Self, Self::Error> {
        match value.0 {
            Message::Text(t) => serde_json::from_slice(t.as_bytes())
                .map_err(WsMessageError::Serde)
                .map(Self::Message),
            Message::Binary(b) => serde_json::from_slice(b.as_ref())
                .map_err(WsMessageError::Serde)
                .map(Self::Message),
            Message::Close(_) => Ok(Self::Close),
            Message::Ping(_) => Err(WsMessageError::UnsupportedOperation),
            Message::Pong(_) => Err(WsMessageError::UnsupportedOperation),
            Message::Frame(_) => Err(WsMessageError::UnsupportedOperation),
        }
    }
}

#[derive(Debug, Error)]
enum WsMessageError {
    #[error("Failed to parse message as JSON: {0}")]
    Serde(serde_json::Error),
    #[error("Unsupported operation")]
    UnsupportedOperation,
}

```