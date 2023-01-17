use clap::Parser;
use std::sync::Arc;

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Cli {

    /// client or server mode
    #[arg(short, long, help = "Run echo server mode", default_value_t=false)]
    pub emode: bool,

    #[arg(short, long, help = "The echo server IP address", default_value_t = String::from("127.0.0.1"))]
    pub server_addr: String,
    
    #[arg(short, long, help= "The echo server UDP port", default_value_t = 7001)]
    pub port: u16,

    #[arg(short, long, help=" How many milliseconds the client waits between sending echo requests to the server", default_value_t = 100)]
    pub interval: u32,

    #[arg(short, long, help=" Use async tasks - else use default of bounded threads for sending and receiving", default_value_t = false)]
   pub  async_mode: bool,

    #[arg(short, long, help= "The http port on which metrics are exported", default_value_t = 8080)]
    pub whttp_port: u16, 
}

pub fn init() -> Arc<Cli> {
    let args = Arc::new(Cli::parse());
    args
}
