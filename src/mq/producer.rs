use super::*;
use lapin::{
    options::{BasicPublishOptions, ExchangeDeclareOptions, QueueDeclareOptions},
    types::FieldTable,
    BasicProperties, Channel,
};
use serde::Serialize;

type ProducerResult<T> = Result<T, MqError>;

#[derive(Debug)]
pub struct Producer<'a> {
    channel: Channel,
    queue: Queue<'a>,
    exchange: Exchange<'a>,
}

impl Producer<'_> {
    pub async fn publish<M: Serialize>(&self, envelope: Envelope<M>) -> ProducerResult<()> {
        let payload = serde_json::to_string(&envelope)?;

        let _result = self
            .channel
            .basic_publish(
                self.exchange.name,
                self.queue.name,
                BasicPublishOptions::default(),
                payload.as_bytes(),
                BasicProperties::default(),
            )
            .await?;

        Ok(())
    }

    pub async fn create<'a>(
        channel: Channel,
        exchange: Exchange<'a>,
        queue: Queue<'a>,
    ) -> ProducerResult<Producer<'a>> {
        channel
            .queue_declare(
                queue.name,
                QueueDeclareOptions::default(),
                FieldTable::default(),
            )
            .await?;

        if exchange.declare {
            channel
                .exchange_declare(
                    exchange.name,
                    lapin::ExchangeKind::Direct,
                    ExchangeDeclareOptions::default(),
                    FieldTable::default(),
                )
                .await?;
        }

        Ok(Producer {
            channel,
            exchange,
            queue,
        })
    }
}
