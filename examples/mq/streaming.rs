use std::time::Duration;

use derive_more::derive::Constructor;
use launchpad::mq::{
    create_channel,
    setup::{ExchangeBuilder, ExchangeType, QueueOptions},
    ChannelOps, CreateChannelConfigFromEnv, Envelope, Exchange, Queue,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::join;
use tracing::info;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

#[tokio::main]
pub async fn main() -> anyhow::Result<()> {
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

async fn topology() -> anyhow::Result<()> {
    use launchpad::mq::setup::{Binding, Exchange, Queue, Topology, TopologyBuilder, TopologyOps};

    let topology = Topology::builder()
        .with_queue(Queue::new(
            "my-queue",
            vec![
                QueueOptions::Persistence(false),
                QueueOptions::AutoExpire(chrono::Duration::minutes(5).num_milliseconds() as u32),
            ],
        ))
        .with_queue(Queue::new("all", vec![]))
        .with_exchange(
            Exchange::builder("streaming-exchange")
                .with_kind(ExchangeType::Topic)
                .with_durable(false)
                .build(),
        )
        .with_binding(Binding::ToQueue {
            src_exchange_name: "streaming-exchange",
            target_queue_name: "my-queue",
            routing_key: Some("person_id.me.item_id.*"),
        })
        .with_binding(Binding::ToQueue {
            src_exchange_name: "streaming-exchange",
            target_queue_name: "all",
            routing_key: Some("person_id.*.item_id.*"),
        })
        .build();

    info!("{}", serde_json::to_string(&topology).unwrap());

    let channel = create_channel(CreateChannelConfigFromEnv).await?;
    channel.apply_topology(topology).await?;

    Ok(())
}

async fn consumer() -> anyhow::Result<()> {
    use futures::stream::StreamExt;
    let channel = create_channel(CreateChannelConfigFromEnv).await?;
    let consumer =
        channel.create_consumer("my-queue-consumer", Queue::new("my-queue"));
    let stream = consumer.stream::<Value>().await?;
    stream.inspect(|item| {
        info!("received: {item:?}");
    }).collect::<Vec<_>>().await;
    info!("consumer complete");
    Ok(())
}

async fn producer() -> anyhow::Result<()> {
    let channel = create_channel(CreateChannelConfigFromEnv).await?;
    let producer = channel.create_producer(Exchange::new("streaming-exchange"));
    #[derive(Debug, Serialize, Deserialize, Constructor)]
    struct Payload {
        value: String,
        person_id: String,
        item_id: String,
    }
    let messages = [
        Payload::new("a".into(), "me".into(), "id1".into()),
        Payload::new("b".into(), "you".into(), "id2".into()),
        Payload::new("c".into(), "me".into(), "id3".into()),
    ]
    .into_iter()
    .map(Envelope::new);

    for message in messages {
        info!("sending message: {:?}", message);
        let rk = format!(
            "person_id.{}.item_id.{}",
            message.message.person_id, message.message.item_id
        );
        producer.publish(message, Some(rk)).await?;
    }
    info!("producing complete");
    Ok(())
}
