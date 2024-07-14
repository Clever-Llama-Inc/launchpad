use std::cell::RefCell;

use derive_more::Constructor;
use lapin::{
    options::{ExchangeBindOptions, ExchangeDeclareOptions, QueueBindOptions, QueueDeclareOptions},
    types::FieldTable,
    Channel,
};
use serde::{Deserialize, Serialize};

use super::MqError;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Topology<Name: Into<String>> {
    queues: Vec<Queue<Name>>,
    exchanges: Vec<Exchange<Name>>,
    bindings: Vec<Binding<Name>>,
}

impl<Name: Into<String>> Topology<Name> {
    pub fn new() -> Self {
        Topology {
            queues: Vec::default(),
            exchanges: Vec::default(),
            bindings: Vec::default(),
        }
    }

    pub fn builder() -> impl TopologyBuilder<Name> {
        RefCell::new(Topology::<Name>::new())
    }
}

pub trait TopologyBuilder<Name: Into<String>> {
    fn with_queue(self, queue: Queue<Name>) -> Self;
    fn with_exchange(self, exchange: Exchange<Name>) -> Self;
    fn with_binding(self, binding: Binding<Name>) -> Self;
    fn build(self) -> Topology<Name>;
}

impl<Name: Into<String>> TopologyBuilder<Name> for RefCell<Topology<Name>> {
    fn with_queue(self, queue: Queue<Name>) -> Self {
        {
            let mut topology = self.borrow_mut();
            topology.queues.push(queue);
        }
        self
    }

    fn with_exchange(self, exchange: Exchange<Name>) -> Self {
        {
            let mut topology = self.borrow_mut();
            topology.exchanges.push(exchange);
        }
        self
    }

    fn with_binding(self, binding: Binding<Name>) -> Self {
        {
            let mut topology = self.borrow_mut();
            topology.bindings.push(binding);
        }
        self
    }

    fn build(self) -> Topology<Name> {
        self.into_inner()
    }
}

#[derive(Debug, Serialize, Deserialize, Constructor)]
pub struct Queue<Name: Into<String>> {
    name: Name,
}

#[derive(Debug, Serialize, Deserialize, Constructor)]
pub struct Exchange<Name: Into<String>> {
    name: Name,
    kind: ExchangeType,
    durable: bool,
}

impl<Name: Into<String>> Exchange<Name> {
    pub fn builder(name: Name) -> impl ExchangeBuilder<Name> {
        RefCell::new(Exchange::<Name>::new(name, ExchangeType::Direct, true))
    }
}

pub trait ExchangeBuilder<Name: Into<String>> {
    fn with_kind(self, kind: ExchangeType) -> Self;
    fn with_durable(self, durable: bool) -> Self;
    fn build(self) -> Exchange<Name>;
}

impl<Name: Into<String>> ExchangeBuilder<Name> for RefCell<Exchange<Name>> {
    fn with_kind(self, kind: ExchangeType) -> Self {
        self.borrow_mut().kind = kind;
        self
    }

    fn with_durable(self, durable: bool) -> Self {
        self.borrow_mut().durable = durable;
        self
    }
    
    fn build(self) -> Exchange<Name> {
        self.into_inner()
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ExchangeType {
    Direct,
    Topic,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Binding<Name: Into<String>> {
    ToQueue {
        src_exchange_name: Name,
        target_queue_name: Name,
        routing_key: Option<Name>,
    },
    ToExchange {
        src_exchange_name: Name,
        target_exchange_name: Name,
        routing_key: Option<Name>,
    },
}

pub trait TopologyOps {
    async fn with_queue<Name: Into<String> + Clone>(
        &self,
        queue: &Queue<Name>,
    ) -> Result<(), MqError>;
    async fn with_exchange<Name: Into<String> + Clone>(
        &self,
        exchange: &Exchange<Name>,
    ) -> Result<(), MqError>;
    async fn with_binding<Name: Into<String> + Clone>(
        &self,
        binding: &Binding<Name>,
    ) -> Result<(), MqError>;

    async fn apply_topology<Name: Into<String> + Clone>(
        &self,
        topology: Topology<Name>,
    ) -> Result<(), MqError> {
        for queue in topology.queues.iter() {
            self.with_queue(&queue).await?
        }

        for exchange in topology.exchanges.iter() {
            self.with_exchange(&exchange).await?
        }

        for binding in topology.bindings.iter() {
            self.with_binding(&binding).await?
        }

        Ok(())
    }
}

impl TopologyOps for Channel {
    async fn with_queue<Name: Into<String> + Clone>(
        &self,
        queue: &Queue<Name>,
    ) -> Result<(), MqError> {
        let queue_name: String = queue.name.clone().into();
        self.queue_declare(
            &queue_name,
            QueueDeclareOptions::default(),
            FieldTable::default(),
        )
        .await?;
        Ok(())
    }

    async fn with_exchange<Name: Into<String> + Clone>(
        &self,
        exchange: &Exchange<Name>,
    ) -> Result<(), MqError> {
        let exchange_name: String = exchange.name.clone().into();
        let exchange_kind = match exchange.kind {
            ExchangeType::Direct => lapin::ExchangeKind::Direct,
            ExchangeType::Topic => lapin::ExchangeKind::Topic,
        };

        let options = ExchangeDeclareOptions {
            durable: exchange.durable,
            ..Default::default()
        };

        self.exchange_declare(
            &exchange_name,
            exchange_kind,
            options,
            FieldTable::default(),
        )
        .await?;
        Ok(())
    }

    async fn with_binding<Name: Into<String> + Clone>(
        &self,
        binding: &Binding<Name>,
    ) -> Result<(), MqError> {
        match binding {
            Binding::ToQueue {
                src_exchange_name,
                target_queue_name,
                routing_key,
            } => {
                let src: String = src_exchange_name.clone().into();
                let dest: String = target_queue_name.clone().into();
                let routing_key: String = routing_key
                    .as_ref()
                    .map(|rk| rk.clone().into())
                    .unwrap_or("".to_string());

                self.queue_bind(
                    &dest,
                    &src,
                    &routing_key,
                    QueueBindOptions::default(),
                    FieldTable::default(),
                )
                .await?;
            }

            Binding::ToExchange {
                src_exchange_name,
                target_exchange_name,
                routing_key,
            } => {
                let src: String = src_exchange_name.clone().into();
                let dest: String = target_exchange_name.clone().into();
                let routing_key: String = routing_key
                    .as_ref()
                    .map(|rk| rk.clone().into())
                    .unwrap_or("".to_string());

                self.exchange_bind(
                    &dest,
                    &src,
                    &routing_key,
                    ExchangeBindOptions::default(),
                    FieldTable::default(),
                )
                .await?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TopologyLogger;
    impl TopologyOps for TopologyLogger {
        async fn with_queue<Name: Into<String> + Clone>(
            &self,
            queue: &Queue<Name>,
        ) -> Result<(), MqError> {
            println!("queue: {}", queue.name.clone().into());
            Ok(())
        }

        async fn with_exchange<Name: Into<String> + Clone>(
            &self,
            exchange: &Exchange<Name>,
        ) -> Result<(), MqError> {
            println!("exchange: {} ({:?})", exchange.name.clone().into(), exchange.kind);
            Ok(())
        }

        async fn with_binding<Name: Into<String> + Clone>(
            &self,
            binding: &Binding<Name>,
        ) -> Result<(), MqError> {
            let (src_name, dest_name, routing_key) = match binding {
                Binding::ToQueue {
                    src_exchange_name,
                    target_queue_name,
                    routing_key,
                } => (
                    src_exchange_name.clone().into(),
                    target_queue_name.clone().into(),
                    routing_key.as_ref().map(|rk| rk.clone().into())
                ),

                Binding::ToExchange {
                    src_exchange_name,
                    target_exchange_name,
                    routing_key,
                } => (
                    src_exchange_name.clone().into(),
                    target_exchange_name.clone().into(),
                    routing_key.as_ref().map(|rk| rk.clone().into())
                ),
            };
            println!("binding: {} -> {}, rk: {:?}", src_name, dest_name, routing_key);
            Ok(())
        }
    }

    #[tokio::test]
    async fn usage() -> anyhow::Result<()> {
        let topology = Topology::builder()
            .with_queue(Queue::new("test.queue"))
            .with_exchange(Exchange::builder("test.exchange").build())
            .with_binding(Binding::ToQueue {
                src_exchange_name: "test.exchange",
                target_queue_name: "test.queue",
                routing_key: None,
            })
            .build();

        TopologyLogger.apply_topology(topology).await?;
        Ok(())
    }
}
