use super::*;
use lapin::{options::BasicPublishOptions, BasicProperties, Channel};
use serde::Serialize;

type ProducerResult<T> = Result<T, MqError>;

#[derive(Debug, Constructor)]
pub struct Producer<'a> {
    channel: Channel,
    exchange: Exchange<'a>,
}

impl Producer<'_> {
    pub async fn publish<M: Serialize>(&self, envelope: Envelope<M>) -> ProducerResult<()> {
        let payload = serde_json::to_string(&envelope)?;

        self
            .channel
            .basic_publish(
                self.exchange.name,
                self.exchange.routing_key.as_ref().map(|e| *e).unwrap_or(""),
                BasicPublishOptions::default(),
                payload.as_bytes(),
                BasicProperties::default(),
            )
            .await?;

        Ok(())
    }
}
