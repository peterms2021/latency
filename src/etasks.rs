use std::{mem, u128};
use std::net::ToSocketAddrs;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::net::UdpSocket;

use crate::eargs;
use crate::emetrics::{Metrics};

pub async fn run_latency_test(args: & Arc<eargs::Cli>, metrics: Arc<Metrics>) ->std::io::Result<()>{
    if args.emode == true {
        let x = Arc::clone(&args);
        let svr = tokio::spawn(async move {
            echo_server(x.port, metrics).await.expect("echo_server task failed");
        });
        svr.await.expect("echo_server task failed");
    } else {
        //check for valid address
        let x = Arc::clone(&args);
        let cli = tokio::spawn(async move {
            echo_client( x.server_addr.clone(), x.port, x.interval, metrics.clone()).await.expect("Client tasks failed to start");
        });
        cli.await.expect("Client tasks failed to start");
    }

    Ok(())
}

async fn echo_server(port: u16, metrics: Arc<Metrics>)-> std::io::Result<()> {
    
    let socket = UdpSocket::bind("0.0.0.0:".to_string() + &port.to_string()).await?;
    println!("\nstart server on port {} \n", port);

    loop {
        let mut buf = vec![0u8; 1000];
        let (amt, src) = socket.recv_from(&mut buf).await?;

#[cfg(test)]
        println!("bytes: {:?} From: {:?}, buf: {:?}",amt, src, &buf[..amt]);

        socket.send_to(&buf[..amt], src).await?;

        metrics.track_server_echoes();
    }
}

async fn echo_client_recv( rx: Arc<UdpSocket>, metrics: Arc<Metrics>) ->std::io::Result<()> {
    println!(" echo_client_recv");

    let mut buf = vec![0u8; 1000];
    loop {

        //let n = rx.recv(&mut buf).await?;
        let (n, addr) = rx.recv_from(&mut buf).await?;

#[cfg(test)]
        {
            println!("{:?} recv from {:?}", n,addr);
            assert_eq!(n, mem::size_of::<u128>());
        }

        let send_time: u128;
        let mut arr = [0u8; 16];
        arr.copy_from_slice(&buf[..n]);
        send_time = u128::from_be_bytes(arr);

        let recv_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
    
        metrics.track_client_rx();

        if recv_time >= send_time {

            let delta = recv_time - send_time;
            let mut dtime: f64 = delta as f64;

            //convert to usec
            dtime = dtime/1000.0;

            #[cfg(test)]
            println!("{:?} detlta(us)", dtime);

            metrics.track_latency(dtime);
        }
    }

// Ok(())
}

async fn echo_client_sender(tx: Arc<UdpSocket>, interval: u32, metrics: Arc<Metrics>)->std::io::Result<()> {
    let mut send_interval = tokio::time::interval(Duration::from_millis(interval as u64));
    loop {
        send_interval.tick().await;

        let time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();

        let bytes = time.to_be_bytes();
        let len = tx.send(&bytes).await?;
        metrics.track_client_tx();

        #[cfg(test)]{
            assert_eq!(len, mem::size_of::<u128>());
            println!("{:?} bytes sent", len);
        }
    }
}

async fn echo_client(addr: String, port: u16, interval: u32, metrics: Arc<Metrics>)->std::io::Result<()>{

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
    let send_metrics = metrics.clone();
    tokio::spawn(async move {
        echo_client_sender(tx,interval, send_metrics).await.expect("echo client sender failed");
    });

    let recv_metrics = metrics.clone();
    tokio::spawn( async move {
        echo_client_recv(rx, recv_metrics).await.expect("client recv tasks failed");
    });

    //spawn the sender
    Ok(())
}
