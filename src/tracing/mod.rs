use std::{collections::BTreeMap, error::Error, process};

use tracing_loki::{url::Url, BackgroundTask};
use tracing_subscriber::{
    fmt,
    layer::{Layer, SubscriberExt},
    prelude::*,
    registry, EnvFilter,
};

#[derive(derive_new::new)]
pub struct LokiOptions {
    #[new(into)]
    url: String,
    #[new(into)]
    labels: BTreeMap<String, String>,
    #[new(into)]
    fields: BTreeMap<String, String>,
}

#[derive(derive_new::new)]
pub struct Logging {
    pub loki_task: Option<BackgroundTask>,
}

pub fn configure(loki: Option<LokiOptions>) -> Result<Logging, Box<dyn Error>> {
    let log_layer = Some(fmt::layer()).with_filter(EnvFilter::from_default_env());
    let mut loki_task = None;

    let loki_layer = if let Some(loki) = loki {
        let mut builder = tracing_loki::builder()
            .label("host", hostname::get()?.to_string_lossy())?
            .extra_field("pid", format!("{}", process::id()))?;

        for (k, v) in loki.labels {
            builder = builder.label(&k, &v)?;
        }

        for (k, v) in loki.fields {
            builder = builder.extra_field(&k, &v)?;
        }

        let (layer, task) = builder.build_url(Url::parse(&loki.url)?)?;

        loki_task = Some(task);

        Some(layer.with_filter(EnvFilter::from_default_env()))
    } else {
        None
    };

    registry().with(log_layer).with(loki_layer).try_init()?;

    Ok(Logging::new(loki_task))
}
