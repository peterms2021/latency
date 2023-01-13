use std::sync::Arc;

mod etasks;
mod ethreaded;
mod emetrics;
mod eargs;

#[tokio::main]
async fn main() {

    let args = &eargs::init();

    let metrics = Arc::new(emetrics::new());
    metrics.register();

    if args.async_mode {
        etasks::run_latency_test(args, metrics.clone()).await;   
    } else {
        ethreaded::run_latency_test(args, metrics.clone());   
    }

    emetrics::run_metrics(args, metrics).await;
}
