use std::env;

use derive_more::{Constructor, Display};
use lapin::{
    options::{
        BasicAckOptions, BasicConsumeOptions, BasicNackOptions, BasicPublishOptions, ExchangeDeclareOptions,
        QueueDeclareOptions,
    },
    types::FieldTable,
    BasicProperties, Channel, Connection, ConnectionProperties,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{debug, warn};

#[derive(Debug, Display)]
pub enum CreateChannelError {
    ConfigError(String),
    ConnectError(String),
    ChannelError(String),
}

pub trait CreateChannelConfig {
    fn rabbitmq_url(&self) -> Result<String, CreateChannelError>;
}

#[derive(Default)]
pub struct CreateChannelConfigFromEnv;

impl CreateChannelConfig for CreateChannelConfigFromEnv {
    fn rabbitmq_url(&self) -> Result<String, CreateChannelError> {
        env::var("RABBITMQ_URL").map_err(|_| CreateChannelError::ConfigError("RABBITMQ_URL must be set".into()))
    }
}

pub async fn create_channel<C: CreateChannelConfig>(config: C) -> Result<Channel, CreateChannelError> {
    let rabbitmq_url = config.rabbitmq_url()?;

    let connection = Connection::connect(&rabbitmq_url, ConnectionProperties::default())
        .await
        .map_err(|e| CreateChannelError::ConnectError(e.to_string()))?;

    let channel = connection
        .create_channel()
        .await
        .map_err(|e| CreateChannelError::ChannelError(e.to_string()))?;

    Ok(channel)
}

#[derive(Debug, Display)]
pub enum ProducerError {
    CreateError(String),
    PublishError(String),
}

#[derive(Debug, Constructor)]
pub struct Queue<'a> {
    name: &'a str,
}

#[derive(Debug)]
pub struct Exchange<'a> {
    name: &'a str,
    declare: bool,
}

impl<'a> Default for Exchange<'a> {
    fn default() -> Self {
        Self {
            name: "",
            declare: false,
        }
    }
}

impl<'a> Exchange<'a> {
    #[allow(dead_code)] // future
    fn new(name: &'a str) -> Exchange {
        Exchange {
            name: name,
            declare: true,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Envelope<M> {
    message: M,
}

impl<'a, M> Envelope<M>
where
    M: Serialize,
    M: Deserialize<'a>,
{
    pub fn new(message: M) -> Self {
        Envelope {
            message: message,
        }
    }
}

#[derive(Debug)]
pub struct Producer<'a> {
    channel: Channel,
    queue: Queue<'a>,
    exchange: Exchange<'a>,
}

impl Producer<'_> {
    pub async fn publish<M: Serialize>(&self, envelope: Envelope<M>) -> Result<(), ProducerError> {
        let payload = serde_json::to_string(&envelope).map_err(|e| ProducerError::PublishError(e.to_string()))?;

        let _result = self
            .channel
            .basic_publish(
                self.exchange.name,
                self.queue.name,
                BasicPublishOptions::default(),
                payload.as_bytes(),
                BasicProperties::default(),
            )
            .await
            .map_err(|e| ProducerError::PublishError(e.to_string()))?;

        Ok(())
    }

    pub async fn create<'a>(
        channel: Channel,
        exchange: Exchange<'a>,
        queue: Queue<'a>,
    ) -> Result<Producer<'a>, ProducerError> {
        channel
            .queue_declare(queue.name, QueueDeclareOptions::default(), FieldTable::default())
            .await
            .map_err(|e| ProducerError::CreateError(e.to_string()))?;

        if exchange.declare {
            channel
                .exchange_declare(
                    exchange.name,
                    lapin::ExchangeKind::Direct,
                    ExchangeDeclareOptions::default(),
                    FieldTable::default(),
                )
                .await
                .map_err(|e| ProducerError::CreateError(e.to_string()))?;
        }

        Ok(Producer {
            channel,
            exchange,
            queue,
        })
    }
}

#[derive(Constructor)]
pub struct Consumer<'a> {
    name: &'a str,
    channel: Channel,
    queue: Queue<'a>,
}

#[derive(Debug, Display)]
pub enum ConsumerError {
    CreateError(String),
    ConsumeError(String),
}

impl From<lapin::Error> for ConsumerError {
    fn from(value: lapin::Error) -> Self {
        ConsumerError::ConsumeError(value.to_string())
    }
}

#[derive(Debug, Display)]
pub enum ProcessorError {
    TemporaryError(String),
    PermanentError(String),
}

pub trait Processor {
    async fn process(&mut self, value: Value) -> Result<(), ProcessorError>;
}

impl Consumer<'_> {
    pub async fn create<'a>(name: &'a str, channel: Channel, queue: Queue<'a>) -> Result<Consumer<'a>, ConsumerError> {
        channel
            .queue_declare(queue.name, QueueDeclareOptions::default(), FieldTable::default())
            .await
            .map_err(|e| ConsumerError::CreateError(e.to_string()))?;

        Ok(Consumer {
            name,
            channel,
            queue,
        })
    }

    pub async fn consume<P: Processor>(&self, processor: &mut P) -> Result<(), ConsumerError> {
        use futures::stream::StreamExt;
        let mut consumer = self
            .channel
            .basic_consume(
                self.queue.name,
                self.name,
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await
            .map_err(ConsumerError::from)?;

        loop {
            if let Some(delivery) = consumer.next().await {
                let delivery = delivery.map_err(ConsumerError::from)?;

                let process_result: Result<(), ProcessorError> = {
                    let envelope: Result<Envelope<Value>, ProcessorError> =
                        serde_json::from_slice::<Envelope<Value>>(&delivery.data)
                            .map_err(|e| ProcessorError::PermanentError(e.to_string()));
                    match envelope {
                        Ok(envelope) => {
                            processor.process(envelope.message).await
                        }
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
