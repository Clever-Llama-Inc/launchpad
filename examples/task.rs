use std::{sync::Arc, time::Duration};

use launchpad::task::{start, Startable};
use thiserror::Error;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let task = Arc::new(MyTask { message: "Hello World!"});
    let running_task = start(task).await?;
    running_task.await??;
    Ok(())
}

#[derive(Error, Debug)]
#[error("My Task Failed! {0}")]
struct MyTaskError(String);

struct MyTask {
    message: &'static str
}

impl Startable<MyTaskError> for MyTask {
    async fn start(&self) -> Result<(), MyTaskError> {
        tokio::time::sleep(Duration::from_millis(500)).await;
        println!("task: {}", self.message);
        Ok(())
    }
}