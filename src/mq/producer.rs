use super::*;
use lapin::{options::BasicPublishOptions, BasicProperties, Channel};
use serde::Serialize;

type ProducerResult<T> = Result<T, MqError>;

#[derive(Debug, Clone, Constructor)]
pub struct Producer<'a> {
    channel: Channel,
    exchange: Exchange<'a>,
}

impl Producer<'_> {
    pub async fn publish<M: Serialize, R: Into<String>>(&self, envelope: Envelope<M>, routing_key: Option<R>) -> ProducerResult<()> {
        let payload = serde_json::to_string(&envelope)?;
        let routing_key = routing_key.map(|r| r.into()).unwrap_or("".into());

        self
            .channel
            .basic_publish(
                self.exchange.name,
                &routing_key,
                BasicPublishOptions::default(),
                payload.as_bytes(),
                BasicProperties::default(),
            )
            .await?;

        Ok(())
    }
}
