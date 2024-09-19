use std::time::Duration;

use launchpad::mq::{
    consumer::{Processor, ProcessorError},
    create_channel,
    setup::{ExchangeBuilder, ExchangeType},
    ChannelOps, CreateChannelConfigFromEnv, Envelope, Exchange, Queue,
};
use serde_json::Value;
use tokio::join;
use tracing::info;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    topology().await?;

    let c = tokio::time::timeout(Duration::from_millis(800), consumer());
    let p = producer();

    let (cr, pr) = join!(c, p);
    cr??;
    pr?;

    Ok(())
}

struct LoggingProcessor;
impl Processor for LoggingProcessor {
    async fn process(&mut self, value: Value) -> Result<(), ProcessorError> {
        info!("received: {:?}", value);
        Ok(())
    }
}

async fn topology() -> anyhow::Result<()> {
    use launchpad::mq::setup::{Binding, Exchange, Queue, Topology, TopologyBuilder, TopologyOps};

    let topology = Topology::builder()
        .with_queue(Queue::new("example-queue"))
        .with_exchange(
            Exchange::builder("example-exchange")
                .with_kind(ExchangeType::Topic)
                .with_durable(true)
                .build(),
        )
        .with_binding(Binding::ToQueue {
            src_exchange_name: "example-exchange",
            target_queue_name: "example-queue",
            routing_key: None,
        })
        .build();

    info!("{:?}", serde_json::to_string(&topology).unwrap());

    let channel = create_channel(CreateChannelConfigFromEnv).await?;
    channel.apply_topology(topology).await?;

    Ok(())
}

async fn consumer() -> anyhow::Result<()> {
    let channel = create_channel(CreateChannelConfigFromEnv).await?;
    let consumer = channel.create_consumer("example-queue-consumer", Queue::new("example-queue"));
    let mut processor: LoggingProcessor = LoggingProcessor;
    consumer.consume(&mut processor).await?;
    info!("consumer complete");
    Ok(())
}

async fn producer() -> anyhow::Result<()> {
    let channel = create_channel(CreateChannelConfigFromEnv).await?;
    let producer = channel.create_producer(Exchange::new("example-exchange"));
    let messages = ["a", "b", "c"]
        .into_iter()
        .map(String::from)
        .map(Envelope::new);

    for message in messages {
        info!("sending message: {:?}", message);
        producer.publish(message, None::<&str>).await?;
    }
    info!("producing complete");
    Ok(())
}
