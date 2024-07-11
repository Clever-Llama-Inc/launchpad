use super::*;
use lapin::{
    options::{
        BasicPublishOptions, ExchangeDeclareOptions, QueueBindOptions, QueueDeclareOptions
    },
    types::FieldTable,
    BasicProperties, Channel,
};
use serde::Serialize;

type ProducerResult<T> = Result<T, MqError>;

#[derive(Debug)]
pub struct Producer<'a> {
    channel: Channel,
    queue: Queue<'a>,
    exchange: Option<Exchange<'a>>,
}

impl Producer<'_> {
    pub async fn publish<M: Serialize>(&self, envelope: Envelope<M>) -> ProducerResult<()> {
        let payload = serde_json::to_string(&envelope)?;

        let _result = self
            .channel
            .basic_publish(
                self.exchange.as_ref().map(|e| e.name).unwrap_or(""),
                self.exchange.as_ref().map(|e| e.routing_key).unwrap_or(self.queue.name),
                BasicPublishOptions::default(),
                payload.as_bytes(),
                BasicProperties::default(),
            )
            .await?;

        Ok(())
    }

    pub async fn create<'a>(
        channel: Channel,
        exchange: Option<Exchange<'a>>,
        queue: Queue<'a>,
    ) -> ProducerResult<Producer<'a>> {
        channel
            .queue_declare(
                queue.name,
                QueueDeclareOptions::default(),
                FieldTable::default(),
            )
            .await?;

        if let Some(exchange) = &exchange {
            if exchange.declare {
                channel
                    .exchange_declare(
                        exchange.name,
                        lapin::ExchangeKind::Direct,
                        ExchangeDeclareOptions::default(),
                        FieldTable::default(),
                    )
                    .await?;

                channel
                    .queue_bind(
                        &queue.name,
                        &exchange.name,
                        &exchange.routing_key,
                        QueueBindOptions::default(),
                        FieldTable::default(),
                    )
                    .await?;
            }
        }

        Ok(Producer {
            channel,
            exchange,
            queue,
        })
    }
}
