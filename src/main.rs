#[macro_use]
extern crate lazy_static;
use futures::StreamExt;
use prometheus::{
    HistogramOpts, Histogram, IntCounterVec, IntCounter, IntGauge, Opts, Registry,
};
use std::result::Result;
use warp::{ws::WebSocket, Filter, Rejection, Reply};
use std::sync::Arc;


const LATENCY_CUSTOM_BUCKETS: &[f64; 25] = &[
    5.0, 25.0, 50.0, 75.0,  100.0, 125.0, 150.0, 175.0, 200.0, 250.0, 300.0, 350.0, 400.0,
    500.0, 600.0, 700.0,800.0, 900.0, 1000.0, 2000.0, 3000.0, 4000.0, 5000.0, 6000.0, 7000.0,
];


lazy_static! { 
    pub static ref SERVER_ECHOS: IntCounter =
        IntCounter::new( "latency_server_echos", "Incoming Echo Requests").expect("metric can be created");

    pub static ref TX_CLIENT_ECHOS: IntCounter =
        IntCounter::new( "latency_echo_client_tx", "Transmitted Echo Requests").expect("metric can be created");

    pub static ref RX_CLIENT_ECHOS: IntCounter =
        IntCounter::new( "latency_echo_client_rx", "Received Echo Requests").expect("metric can be created");

    pub static ref LATENCY_COLLECTOR: Histogram = Histogram::with_opts(
        HistogramOpts::new("latency", "Echo Response Times in microseconds").buckets(LATENCY_CUSTOM_BUCKETS.to_vec()))
        .expect("metric can be created"); 

    pub static ref CONNECTED_CLIENTS: IntGauge =
        IntGauge::new( "connected_clients", "Connected Clients").expect("metric can be created");

    pub static ref REGISTRY: Registry = Registry::new();
}


pub fn register_custom_metrics() {

    REGISTRY
            .register(Box::new(SERVER_ECHOS.clone()))
            .expect("collector can be registered");

    REGISTRY
            .register(Box::new(RX_CLIENT_ECHOS.clone()))
            .expect("collector can be registered");

    REGISTRY
            .register(Box::new(TX_CLIENT_ECHOS.clone()))
            .expect("collector can be registered");

    REGISTRY
        .register(Box::new(LATENCY_COLLECTOR.clone()))
        .expect("collector can be registered");

    REGISTRY
            .register(Box::new(CONNECTED_CLIENTS.clone()))
            .expect("collector can be registered");


}


pub fn track_latence(response_time: f64) {
    LATENCY_COLLECTOR .observe(response_time);
}

pub fn track_server_echos(){
    SERVER_ECHOS.inc();
}

pub fn track_client_tx(){
    TX_CLIENT_ECHOS.inc();
}

pub fn track_client_rx(){
    RX_CLIENT_ECHOS.inc();
}


async fn ws_handler(ws: warp::ws::Ws, id: String) -> Result<impl Reply, Rejection> {
    Ok(ws.on_upgrade(move |socket| client_connection(socket, id)))
}


async fn client_connection(ws: WebSocket, id: String) {
    let (_client_ws_sender, mut client_ws_rcv) = ws.split();

    CONNECTED_CLIENTS.inc();
    println!("{} connected", id);

    while let Some(result) = client_ws_rcv.next().await {
        match result {
            Ok(msg) => println!("received message: {:?}", msg),
            Err(e) => {
                eprintln!("error receiving ws message for id: {}): {}", id.clone(), e);
                break;
            }
        };
    }

    println!("{} disconnected", id);
    CONNECTED_CLIENTS.dec();
}


async fn metrics_handler() -> Result<impl Reply, Rejection> {

    use prometheus::Encoder;
    let encoder = prometheus::TextEncoder::new();

    let mut buffer = Vec::new();
    if let Err(e) = encoder.encode(&REGISTRY.gather(), &mut buffer) {
        eprintln!("could not encode custom metrics: {}", e);
    };
    let mut res = match String::from_utf8(buffer.clone()) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("custom metrics could not be from_utf8'd: {}", e);
            String::default()
        }
    };
    buffer.clear();

    let mut buffer = Vec::new();
    if let Err(e) = encoder.encode(&prometheus::gather(), &mut buffer) {
        eprintln!("could not encode prometheus metrics: {}", e);
    };
    let res_custom = match String::from_utf8(buffer.clone()) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("prometheus metrics could not be from_utf8'd: {}", e);
            String::default()
        }
    };
    buffer.clear();

    res.push_str(&res_custom);
    Ok(res)
}

use crate::eargs;

pub async fn run(args: &Arc<eargs::Cli>)
{
  
    //set up the web interface for prometheus data export and a generic 
    let metrics_route = warp::path!("metrics").and_then(metrics_handler);
    let ws_route = warp::path("ws")
        .and(warp::ws())
        .and(warp::path::param())
        .and_then(ws_handler);
    
    println!("Started webservice on port {}", args.whttp_port);

    //only start the metrics service for client mode
   if args.emode == false 
    {
            warp::serve(metrics_route.or(ws_route))
                .run(([0, 0, 0, 0], args.whttp_port))
                .await;

    }  
}


mod emetrics;
mod etasks;
mod ethreaded;
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

    emetrics::run(args);
}



////////