use std::time::{Duration, Instant};

use rocket::{
    fairing::{Fairing, Info, Kind},
    Data, Request, Response,
};
use tracing::{info, warn};

pub struct Metrics;


#[rocket::async_trait]
impl Fairing for Metrics {
    fn info(&self) -> Info {
        Info {
            name: "metrics",
            kind: Kind::Request | Kind::Response,
        }
    }

    async fn on_request(&self, req: &mut Request<'_>, _data: &mut Data<'_>) {
        req.local_cache(|| Some(RequestTimer::new()));
    }

    async fn on_response<'r>(&self, req: &'r Request<'_>, res: &mut Response<'r>) {
        if let Some(timer) = req.local_cache(|| <Option<RequestTimer>>::None) {
            let elapsed = timer.elapsed().as_secs_f32();
            let method = req.method().as_str();
            let path = req.uri().path().to_string();
            let status = res.status().code;
            info!(target: "http.metrics", elapsed, method, path, status);
        } else {
            warn!("request timer not found.");
        }
    }
}

struct RequestTimer {
    start: Instant
}

impl RequestTimer {
    fn new() -> Self {
        Self {
            start: Instant::now()
        }
    }

    fn elapsed(&self) -> Duration {
        Instant::now() - self.start
    }
}
