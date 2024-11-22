#[cfg(feature = "mq")]
mod basic;
#[cfg(feature = "mq")]
mod streaming;

#[cfg(feature = "mq")]
#[tokio::main]
async fn main() -> anyhow::Result<()>{
    basic::main()?;
    streaming::main()?;
    Ok(())
}

#[cfg(not(feature = "mq"))]
fn main() {
    println!("'mq' feature is disabled")
}