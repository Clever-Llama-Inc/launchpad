use launchpad::mq::{
    consumer::{Consumer, Processor, ProcessorError},
    create_channel,
    producer::Producer,
    CreateChannelConfigFromEnv, Envelope, Exchange, Queue,
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

    let (cr, pr) = join!(consumer(), producer());
    cr?;
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

async fn consumer() -> anyhow::Result<()> {
    let channel = create_channel(CreateChannelConfigFromEnv).await?;
    let queue = Queue::new("example-queue");
    let consumer = Consumer::create("example-consumer", channel, queue).await?;
    let mut processor = LoggingProcessor;
    consumer.consume(&mut processor).await?;
    info!("consumer complete");
    Ok(())
}

async fn producer() -> anyhow::Result<()> {
    let channel = create_channel(CreateChannelConfigFromEnv).await?;
    let queue = Queue::new("example-queue");
    let exchange = Exchange {
        name: "example-exchange",
        declare: true,
        routing_key: "",
    };
    let producer = Producer::create(channel, Some(exchange), queue).await?;
    let messages = ["a", "b", "c"]
        .into_iter()
        .map(String::from)
        .map(Envelope::new);
    for message in messages {
        info!("sending message: {:?}", message);
        producer.publish(message).await?;
    }
    info!("producing complete");
    Ok(())
}
