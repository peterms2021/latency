use crate::eargs;

use futures::StreamExt;
use prometheus::{
    Histogram, HistogramOpts, IntCounter, IntGauge, Registry,
    proto::MetricFamily,
};
use std::result::Result;
use warp::{ws::WebSocket, Filter, Rejection, Reply};
use std::sync::Arc;

pub struct Metrics {
    registry: Registry,
    latency_server_echoes: Box<IntCounter>,
    latency_echo_client_rx: Box<IntCounter>,
    latency_echo_client_tx: Box<IntCounter>,
    latency: Box<Histogram>,
    connected_clients: Box<IntGauge>,
}

pub fn new() -> Metrics  {
    const LATENCY_CUSTOM_BUCKETS: &[f64; 25] = &[
        5.0, 25.0, 50.0, 75.0,  100.0, 125.0, 150.0, 175.0, 200.0, 250.0, 300.0, 350.0, 400.0,
        500.0, 600.0, 700.0,800.0, 900.0, 1000.0, 2000.0, 3000.0, 4000.0, 5000.0, 6000.0, 7000.0,
    ];
    let histogram_opts =
        HistogramOpts::new("latency", "Echo Response Times in microseconds")
            .buckets(LATENCY_CUSTOM_BUCKETS.to_vec());
    return Metrics{
        registry : Registry::new(),
        latency_server_echoes: Box::new(IntCounter::new( "latency_server_echos", "Incoming Echo Requests").unwrap()),
        latency_echo_client_rx: Box::new(IntCounter::new( "latency_echo_client_rx", "Received Echo Requests").unwrap()),
        latency_echo_client_tx: Box::new(IntCounter::new( "latency_echo_client_tx", "Transmitted Echo Requests").unwrap()),
        latency: Box::new(Histogram::with_opts(histogram_opts).unwrap()),
        connected_clients: Box::new(IntGauge::new( "connected_clients", "Connected Clients").unwrap()),
    };
}

impl Metrics {
    pub fn dec_client(&self) {
        self.connected_clients.dec()
    }

    pub fn gather(&self) -> Vec<MetricFamily> {
        return self.registry.gather();
    }

    pub fn inc_client(&self) {
        self.connected_clients.inc()
    }

    pub fn register(&self) {
        self.registry.register(self.latency_server_echoes.clone()).unwrap();
        self.registry.register(self.latency_echo_client_rx.clone()).unwrap();
        self.registry.register(self.latency_echo_client_tx.clone()).unwrap();
        self.registry.register(self.latency.clone()).unwrap();
        self.registry.register(self.connected_clients.clone()).unwrap();
    }

    pub fn track_client_rx(&self) {
        self.latency_echo_client_rx.inc()
    }

    pub fn track_client_tx(&self) {
        self.latency_echo_client_tx.inc()
    }

    pub fn track_latency(&self, response_time: f64) {
        self.latency.observe(response_time)
    }

    pub fn track_server_echoes(&self) {
        self.latency_server_echoes.inc();
    }
}

unsafe impl Sync for Metrics {}

async fn ws_handler(ws: warp::ws::Ws, id: String, metrics: Arc<Metrics>) -> Result<impl Reply, Rejection> {
    Ok(ws.on_upgrade(move |socket| client_connection(socket, id, metrics)))
}

async fn client_connection(ws: WebSocket, id: String, metrics: Arc<Metrics>) {
    let (_client_ws_sender, mut client_ws_rcv) = ws.split();

    metrics.inc_client();
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
    metrics.dec_client();
}


async fn metrics_handler(metrics: Arc<Metrics>) -> Result<impl Reply, Rejection> {

    use prometheus::Encoder;
    let encoder = prometheus::TextEncoder::new();

    let mut buffer = Vec::new();
    if let Err(e) = encoder.encode(&metrics.gather(), &mut buffer) {
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

pub fn with_metrics(metrics: Arc<Metrics>) -> impl Filter<Extract = (Arc<Metrics>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || metrics.clone())
}

pub async fn run_metrics(args: &Arc<eargs::Cli>, metrics: Arc<Metrics>)
{
  
    //set up the web interface for prometheus data export and a generic 
    let metrics_route = warp::path!("metrics")
        .and(with_metrics(metrics.clone()))
        .and_then(metrics_handler);
    let ws_route = warp::path("ws")
        .and(warp::ws())
        .and(warp::path::param())
        .and(with_metrics(metrics))
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
