pub mod consumer;
pub mod producer;
pub mod setup;

use std::env;

use consumer::Consumer;
use derive_more::{Constructor, From};
use lapin::{Channel, Connection, ConnectionProperties};
use producer::Producer;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MqError {
    #[error("Configuration Error: {0}")]
    ConfigurationError(String),

    #[error("Lapin Error: {0}")]
    LapinError(#[from] lapin::Error),

    #[error("Serde JSON Error: {0}")]
    SerdeJsonError(#[from] serde_json::Error),
}

pub trait CreateChannelConfig {
    fn rabbitmq_url(&self) -> Result<String, MqError>;
}

pub struct CreateChannelConfigFromEnv;

impl CreateChannelConfig for CreateChannelConfigFromEnv {
    fn rabbitmq_url(&self) -> Result<String, MqError> {
        env::var("RABBITMQ_URL")
            .map_err(|_| MqError::ConfigurationError("RABBITMQ_URL must be set".into()))
    }
}

pub async fn create_channel<C: CreateChannelConfig>(config: C) -> Result<Channel, MqError> {
    let rabbitmq_url = config.rabbitmq_url()?;
    let connection = Connection::connect(&rabbitmq_url, ConnectionProperties::default()).await?;
    let channel = connection.create_channel().await?;

    Ok(channel)
}

pub trait ChannelOps {
    fn create_producer(self, exchange: Exchange<'_>) -> Producer<'_>;
    fn create_consumer<'a>(self, consumer_tag: &'a str, queue: Queue<'a>) -> Consumer<'a>;
}

impl ChannelOps for Channel {
    fn create_producer(self, exchange: Exchange<'_>) -> Producer<'_> {
        Producer::new(self, exchange)
    }

    fn create_consumer<'a>(self, consumer_tag: &'a str, queue: Queue<'a>) -> Consumer<'a> {
        Consumer::new(self, consumer_tag, queue)
    }
}

#[derive(Debug, Constructor, From)]
pub struct Queue<'a> {
    pub name: &'a str,
}

#[derive(Debug, Constructor, From)]
pub struct Exchange<'a> {
    pub name: &'a str,
    pub routing_key: Option<&'a str>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Envelope<M> {
    pub message: M,
}

impl<M> Envelope<M>
where
    M: Serialize,
    M: DeserializeOwned,
{
    pub fn new(message: M) -> Self {
        Envelope { message }
    }
}
