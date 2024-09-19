use std::pin::Pin;

use super::*;
use futures::{future, Stream, StreamExt, TryStreamExt};
use lapin::{
    message::Delivery,
    options::{BasicAckOptions, BasicConsumeOptions, BasicNackOptions},
    types::FieldTable,
    Channel,
};
use serde_json::Value;
use thiserror::Error;
use tracing::{debug, warn};

pub type ConsumerResult<T> = Result<T, MqError>;
pub type ConsumerStream<Item> = Pin<Box<dyn Stream<Item = Item>>>;

#[derive(Constructor, Clone)]
pub struct Consumer<'a> {
    channel: Channel,
    consumer_tag: &'a str,
    queue: Queue<'a>,
}

#[derive(Error, Debug)]
pub enum ProcessorError {
    #[error("Temporary Error: {0}")]
    TemporaryError(String),

    #[error("Permanent Error: {0}")]
    PermanentError(String),
}

pub trait Processor {
    async fn process(&mut self, value: Value) -> Result<(), ProcessorError>;
}

impl Consumer<'_> {
    pub async fn consume<P: Processor>(&self, processor: &mut P) -> ConsumerResult<()> {
        use futures::stream::StreamExt;
        let mut consumer = self
            .channel
            .basic_consume(
                self.queue.name,
                self.consumer_tag,
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await?;

        loop {
            if let Some(delivery) = consumer.next().await {
                let delivery = delivery?;

                let process_result: Result<(), ProcessorError> = {
                    let envelope: Result<Envelope<Value>, ProcessorError> =
                        serde_json::from_slice::<Envelope<Value>>(&delivery.data)
                            .map_err(|e| ProcessorError::PermanentError(e.to_string()));
                    match envelope {
                        Ok(envelope) => processor.process(envelope.message).await,
                        Err(e) => Err(e),
                    }
                };

                handle_message_result(&delivery, &process_result).await?;
            } else {
                warn!("no message, finishing");
                break;
            }
        }

        Ok(())
    }

    pub async fn stream<Item>(&self) -> ConsumerResult<ConsumerStream<Item>>
    where
        Item: DeserializeOwned,
    {
        let consumer = self
            .channel
            .basic_consume(
                self.queue.name,
                self.consumer_tag,
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await?;

        let stream = consumer
            .inspect_err(|e| warn!("error consuming: {:?}", e))
            .take_while(|d| future::ready(d.is_ok()))
            .map_err(MqError::from)
            .map(|d| d.unwrap())
            .then(|d| async move {
                match serde_json::from_slice::<Envelope<Item>>(&d.data).map_err(MqError::from) {
                    Ok(Envelope { message }) => {
                        handle_message_result(&d, &Ok(())).await?;
                        Ok(message)
                    },
                    Err(e) => {
                        handle_message_result(&d, &Err(ProcessorError::PermanentError(e.to_string()))).await?;
                        Err(e)
                    },
                }
            })
            .inspect_err(|e| warn!("error extracting message: {:?}", e))
            .take_while(|i| future::ready(i.is_ok()))
            .map(|i| i.unwrap());

        Ok(Box::pin(stream))
    }
}

async fn handle_message_result(
    delivery: &Delivery,
    result: &Result<(), ProcessorError>,
) -> ConsumerResult<()> {
    match result {
        Ok(_) => {
            debug!("message successful");
            delivery.ack(BasicAckOptions::default()).await?
        }
        Err(e @ ProcessorError::TemporaryError(_)) => {
            warn!("message failed temporarily: {:?}", e);
            delivery
                .nack(BasicNackOptions {
                    multiple: false,
                    requeue: true,
                })
                .await?
        }
        Err(e @ ProcessorError::PermanentError(_)) => {
            warn!("message failed permanently: {:?}", e);
            delivery
                .nack(BasicNackOptions {
                    multiple: false,
                    requeue: false,
                })
                .await?
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use futures::StreamExt;
    use serde::Deserialize;

    use super::{create_channel, ChannelOps, CreateChannelConfigFromEnv};

    async fn _stream_usage() -> anyhow::Result<()> {
        #[derive(Debug, Deserialize)]
        struct Usage {
            pub _id: i32,
        }
        let channel = create_channel(CreateChannelConfigFromEnv).await?;
        let consumer = channel.create_consumer("usage-consumer", "usage-queue".into());
        let stream = consumer.stream::<Usage>().await?;
        let _messages = stream
            .take(5)
            .map(|e| {
                println!("Received: {:?}", e);
                e
            })
            .collect::<Vec<_>>()
            .await;
        Ok(())
    }
}
