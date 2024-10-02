use std::{error::Error, process};

use tracing_loki::url::Url;
use tracing_subscriber::{
    fmt,
    layer::{Layer, SubscriberExt},
    prelude::*,
    registry, EnvFilter,
};

pub fn configure(loki_url: Option<String>) -> Result<(), Box<dyn Error>> {
    let log_layer = Some(fmt::layer()).with_filter(EnvFilter::from_default_env());

    let loki_layer = if let Some(loki_url) = loki_url {
        let (layer, task) = tracing_loki::builder()
            .label("host", hostname::get()?.to_string_lossy())?
            .extra_field("pid", format!("{}", process::id()))?
            .build_url(Url::parse(&loki_url)?)?;

        tokio::spawn(task);
        
        Some(layer)
    } else {
        None
    };

    registry()
        .with(log_layer)
        .with(loki_layer)
        .try_init()?;

    Ok(())
}
