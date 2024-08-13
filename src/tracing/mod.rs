use tracing_subscriber::{fmt, prelude::*, EnvFilter};

pub fn configure() {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();
}
