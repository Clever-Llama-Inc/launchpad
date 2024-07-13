use std::sync::Arc;

use tokio::task::JoinHandle;

pub trait Startable<E> {
    fn start(&self) -> impl std::future::Future<Output = Result<(), E>> + std::marker::Send;
}

pub async fn start<E: Send + Sync + 'static, S: Startable<E> + Send + Sync + 'static>(
    startable: Arc<S>,
) -> Result<JoinHandle<Result<(), E>>, E> {
    let startable = Arc::clone(&startable);
    let join: JoinHandle<Result<(), E>> = tokio::spawn(async move { startable.start().await });
    Ok(join)
}