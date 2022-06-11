pub mod commander;
pub mod conf;
pub mod detectors;
pub mod grpc;
pub mod reporter;
pub mod structures;

#[macro_export]
macro_rules! backoff {
    ($min:expr, $max:expr) => {{
        use rand::Rng;
        use tokio::time::{self, Duration};
        use tracing::info;

        let rand_num = rand::thread_rng().gen_range($min..=$max);
        info!("Wait {} sec retry", rand_num);

        let wait = Duration::from_secs(rand_num);
        time::sleep(wait).await;
    }};
}
