use super::*;
use derive_more::Constructor;
use lapin::{
    options::{BasicAckOptions, BasicConsumeOptions, BasicNackOptions, QueueDeclareOptions},
    types::FieldTable,
    Channel,
};
use serde_json::Value;
use thiserror::Error;
use tracing::{debug, warn};

type ConsumerResult<T> = Result<T, MqError>;

#[derive(Constructor)]
pub struct Consumer<'a> {
    name: &'a str,
    channel: Channel,
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
    pub async fn create<'a>(
        name: &'a str,
        channel: Channel,
        queue: Queue<'a>,
    ) -> ConsumerResult<Consumer<'a>> {
        channel
            .queue_declare(
                queue.name,
                QueueDeclareOptions::default(),
                FieldTable::default(),
            )
            .await?;

        Ok(Consumer {
            name,
            channel,
            queue,
        })
    }

    pub async fn consume<P: Processor>(&self, processor: &mut P) -> ConsumerResult<()> {
        use futures::stream::StreamExt;
        let mut consumer = self
            .channel
            .basic_consume(
                self.queue.name,
                self.name,
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

                let _ackr = match process_result {
                    Ok(_) => {
                        debug!("message successful");
                        delivery.ack(BasicAckOptions::default()).await
                    }
                    Err(e @ ProcessorError::TemporaryError(_)) => {
                        warn!("message failed temporarily: {:?}", e);
                        delivery
                            .nack(BasicNackOptions {
                                multiple: false,
                                requeue: true,
                            })
                            .await
                    }
                    Err(e @ ProcessorError::PermanentError(_)) => {
                        warn!("message failed permanently: {:?}", e);
                        delivery
                            .nack(BasicNackOptions {
                                multiple: false,
                                requeue: false,
                            })
                            .await
                    }
                };
            } else {
                warn!("no message, finishing");
                break;
            }
        }

        Ok(())
    }
}
