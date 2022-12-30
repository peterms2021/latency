#[macro_use]
extern crate lazy_static;
use futures::StreamExt;
use prometheus::{
    HistogramOpts, Histogram, IntCounterVec, IntCounter, IntGauge, Opts, Registry,
};
use rand::{thread_rng, Rng};
use std::result::Result;
use std::time::Duration;
use warp::{ws::WebSocket, Filter, Rejection, Reply};
use tokio::{net::UdpSocket};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use std::sync::Arc;
use std::{u128 };
use std::net::{ ToSocketAddrs};
use std::mem;


lazy_static! {
    pub static ref INCOMING_REQUESTS: IntCounter =
        IntCounter::new( "incoming_requests", "Incoming Requests").expect("metric can be created");

    pub static ref SERVER_ECHOS: IntCounter =
        IntCounter::new( "echo_requests", "Incoming Echo Requests").expect("metric can be created");

    pub static ref CLIENT_ECHOS: IntCounter =
        IntCounter::new( "echo_sent", "Transmitted Echo Requests").expect("metric can be created");

    pub static ref CONNECTED_CLIENTS: IntGauge =
        IntGauge::new( "connected_clients", "Connected Clients").expect("metric can be created");

    pub static ref RESPONSE_CODE_COLLECTOR: IntCounterVec = IntCounterVec::new(
        Opts::new("response_code", "Response Codes"),
        &["statuscode", "type"]
    ) .expect("metric can be created");

    pub static ref RESPONSE_TIME_COLLECTOR: Histogram = Histogram::with_opts(
        HistogramOpts::new("latency", "Echo Response Times")
    ).expect("metric can be created");

    pub static ref LATENCY_COLLECTOR: Histogram = Histogram::with_opts(
        HistogramOpts::new("latency", "Echo Response Times")
    ).expect("metric can be created"); 

    pub static ref REGISTRY: Registry = Registry::new();
}


#[cfg(test)]
lazy_static! {
    pub static ref TEST_LATENCY_COLLECTOR: Histogram = Histogram::with_opts(
        HistogramOpts::new("test_latency", "Server Delay Response Times")
    ).expect("metric can be created"); 
}

use clap::Parser;
#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// client or server mode
    #[arg(short, long, help = "Run echo server mode", default_value_t=false)]
    emode: bool,

    /// Optional name to operate on
    #[arg(short, long, help = "The echo server IP address", default_value_t = String::from("127.0.0.1"))]
    server_addr: String,
    
    /// Sets a custom config file
    #[arg(short, long, help= "The echo server UDP port", default_value_t = 7001)]
    port: u16,

    /// Sets a custom config file
    #[arg(short, long, help=" How many milliseconds the client waits between sending echo requests to the server", default_value_t = 100)]
    interval: u32,

    #[arg(short, long, help= "The http port on which metrics are exported", default_value_t = 8080)]
    whttp_port: u16, 
}


fn register_custom_metrics() {
    REGISTRY
        .register(Box::new(INCOMING_REQUESTS.clone()))
        .expect("collector can be registered");

    REGISTRY
        .register(Box::new(CONNECTED_CLIENTS.clone()))
        .expect("collector can be registered");

    REGISTRY
            .register(Box::new(SERVER_ECHOS.clone()))
            .expect("collector can be registered");

    REGISTRY
            .register(Box::new(CLIENT_ECHOS.clone()))
            .expect("collector can be registered");

    REGISTRY
        .register(Box::new(RESPONSE_CODE_COLLECTOR.clone()))
        .expect("collector can be registered");

    REGISTRY
        .register(Box::new(RESPONSE_TIME_COLLECTOR.clone()))
        .expect("collector can be registered");

#[cfg(test)]
    REGISTRY
        .register(Box::new(TEST_LATENCY_COLLECTOR.clone()))
        .expect("collector can be registered");

}

fn track_request_time(response_time: f64) {
    RESPONSE_TIME_COLLECTOR .observe(response_time);
}

#[cfg(test)]
/// all of this code is demo code for prometheus
async fn some_handler() -> Result<impl Reply, Rejection> {
    INCOMING_REQUESTS.inc();
    Ok("hello!")
}

async fn simulated_data_collector() {
    let mut collect_interval = tokio::time::interval(Duration::from_millis(10));
    loop {
        collect_interval.tick().await;
        let mut rng = thread_rng();
        let response_time: f64 = rng.gen_range(0.001..10.0);
        let response_code: usize = rng.gen_range(100..599);

        track_status_code(response_code);
        track_request_time(response_time );
    }
}

#[cfg(test)]
async fn induce_delay() {
    let mut rng = thread_rng();
    let delay_time: u32 = rng.gen_range(10..1000);
    let mut collect_interval = tokio::time::interval(Duration::from_micros(delay_time as u64));
    collect_interval.tick().await;
    TEST_LATENCY_COLLECTOR.observe(delay_time as f64);
}

fn track_status_code(status_code: usize) {
    match status_code {
        500..=599 => RESPONSE_CODE_COLLECTOR
            .with_label_values(&[&status_code.to_string(), "500"])
            .inc(),
        400..=499 => RESPONSE_CODE_COLLECTOR
            .with_label_values(&[&status_code.to_string(), "400"])
            .inc(),
        300..=399 => RESPONSE_CODE_COLLECTOR
            .with_label_values(&[&status_code.to_string(), "300"])
            .inc(),
        200..=299 => RESPONSE_CODE_COLLECTOR
            .with_label_values(&[&status_code.to_string(), "200"])
            .inc(),
        100..=199 => RESPONSE_CODE_COLLECTOR
            .with_label_values(&[&status_code.to_string(), "100"])
            .inc(),
        _ => (),
    };
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

async fn echo_server(port: u16)-> std::io::Result<()> {
    
    let socket = UdpSocket::bind("0.0.0.0:".to_string() + &port.to_string()).await?;
    println!("\nstart server on port {} \n", port);

    loop {
        let mut buf = [0; 10];
        let (_amt, src) = socket.recv_from(&mut buf).await?;
         println!("bytes: {:?} From: {:?}, buf: {:?}",_amt, src, buf);
        socket.send_to(&buf, src).await?;
        SERVER_ECHOS.inc();
    }
}

async fn echo_client_recv( rx: Arc<UdpSocket>) ->std::io::Result<()> {
    println!(" echo_client_recv");

    let mut buf = vec![0u8, 100];
    loop {

        //let n = rx.recv(&mut buf).await?;
        let (n, addr) = rx.recv_from(&mut buf).await?;

        println!("{:?} recv from {:?}", n,addr);
        assert_eq!(n, mem::size_of::<u128>());

        let send_time: u128;
        let mut arr = [0u8; 16];
        arr.copy_from_slice(&buf[..n]);
        send_time = u128::from_be_bytes(arr);

#[cfg(test2)]
    {
        {
            let rng = thread_rng();
            let delay_time: u32 = rng.gen_range(10..1000);
            let collect_interval = tokio::time::interval(Duration::from_micros(delay_time as u64));
            let dtime = delay_time as f64;

            TEST_LATENCY_COLLECTOR.observe( dtime );
            collect_interval.tick().await;
        }
    }


        let recv_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();

        if recv_time >= send_time {

            let mut delta = recv_time - send_time;
            println!("{:?} detlta(ns)", delta);

            //convert to usec
            delta = delta/1000;
            println!("{:?} detlta(us)", delta);

            let dtime: f64 = delta as f64;
            LATENCY_COLLECTOR.observe(dtime);
        }
    }

   // Ok(())
}

async fn echo_client_sender(tx: Arc<UdpSocket>, interval: u32)->std::io::Result<()> {
    let mut send_interval = tokio::time::interval(Duration::from_millis(interval as u64));
    loop {
        send_interval.tick().await;

        let time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();

        let bytes = time.to_be_bytes();
        let len = tx.send(&bytes).await?;
            assert_eq!(len, mem::size_of::<u128>());
            println!("{:?} bytes sent", len);
    }
}

async fn echo_client(addr: String, port: u16, interval: u32)->std::io::Result<()>{

    let ad = addr + ":" + &port.to_string();
    let _remote_addr_iter= ad.to_socket_addrs().expect("Unable to resolve domain");

    println!("Remote addr {}",ad);

    // We use port 0 to let the operating system allocate an available port for us.
    let local_addr  = "0.0.0.0:0".to_string();

    let sock = UdpSocket::bind(local_addr).await?;
    //let send_addr = addr + ":" + &port.to_string();
    sock.connect(ad).await?;
 
    let rx = Arc::new(sock);
    let tx = rx.clone();    

    //spawn thread that will wait for the echos
    tokio::spawn(async  move  {
        echo_client_sender(tx,interval).await.expect("echo client sender failed");
    });

    tokio::spawn( async move {
        echo_client_recv(rx).await.expect("client recv tasks failed");
    });

    //spawn the sender
    Ok(())
}


#[tokio::main]
async fn main() {

    let args = Arc::new(Cli::parse());

    register_custom_metrics();

    if args.emode == true {
        let x = Arc::clone(&args);
        let svr = tokio::spawn(async move {
            echo_server(x.port).await.expect("echo_server task failed");
        });
        svr.await.expect("echo_server task failed");
    }else{
        //check for valid address
        let x = Arc::clone(&args);
        let cli = tokio::spawn(async move {
            echo_client( x.server_addr.clone(), x.port, x.interval).await.expect("Client tasks failed to start");
        });
        cli.await.expect("Client tasks failed to start");
    }


    //set up the web interface for prometheus data export and a generic 
    let metrics_route = warp::path!("metrics").and_then(metrics_handler);
    let ws_route = warp::path("ws")
        .and(warp::ws())
        .and(warp::path::param())
        .and_then(ws_handler);
    
    println!("Started webservice on port {}", args.whttp_port);

#[cfg(test)]
 {
    let some_route = warp::path!("some").and_then(some_handler);
    //kick off the data collector tasks    
    tokio::task::spawn(simulated_data_collector());

    warp::serve(metrics_route.or(some_route).or(ws_route))
        .run(([0, 0, 0, 0], args.whttp_port))
        .await;
}

#[cfg(not(test))]   {
    warp::serve(metrics_route.or(ws_route))
        .run(([0, 0, 0, 0], args.whttp_port))
        .await;

}
}



////////