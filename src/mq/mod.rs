pub mod consumer;
pub mod producer;

use std::env;

use derive_more::Constructor;
use lapin::{Channel, Connection, ConnectionProperties};
use serde::{Deserialize, Serialize};
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

#[derive(Debug, Constructor)]
pub struct Queue<'a> {
    pub name: &'a str,
}

#[derive(Debug)]
pub struct Exchange<'a> {
    pub name: &'a str,
    pub declare: bool,
    pub routing_key: &'a str,
}

impl<'a> Exchange<'a> {
    #[allow(dead_code)] // future
    pub fn new(name: &'a str) -> Exchange {
        Exchange {
            name: name,
            declare: true,
            routing_key: ""
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Envelope<M> {
    pub message: M,
}

impl<'a, M> Envelope<M>
where
    M: Serialize,
    M: Deserialize<'a>,
{
    pub fn new(message: M) -> Self {
        Envelope { message: message }
    }
}
