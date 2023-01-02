#[macro_use]
extern crate lazy_static;


mod etasks;
mod ethreaded;
mod emetrics;
mod eargs;

#[tokio::main]
async fn main() {

    let args = &eargs::init();

    emetrics::register_custom_metrics();

    if args.async_mode {
        etasks::run_latency_test(args);   
    }else{
        ethreaded::run_latency_test(args);   
    }

    emetrics::run_metrics(args);
}



////////