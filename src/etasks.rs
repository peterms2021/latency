
use std::time::Duration;
use tokio::{net::UdpSocket};
use std::time::{SystemTime, UNIX_EPOCH};
use std::sync::Arc;
use std::{u128 };
use std::net::{ ToSocketAddrs};
use std::mem;

use crate::eargs;
use crate::emetrics;

async fn echo_server(port: u16)-> std::io::Result<()> {
    
    let socket = UdpSocket::bind("0.0.0.0:".to_string() + &port.to_string()).await?;
    println!("\nstart server on port {} \n", port);

    loop {
        let mut buf = vec![0u8; 1000];
        let (amt, src) = socket.recv_from(&mut buf).await?;

#[cfg(test)]
        println!("bytes: {:?} From: {:?}, buf: {:?}",amt, src, &buf[..amt]);

        socket.send_to(&buf[..amt], src).await?;

        emetrics::track_server_echos();
    }
}

async fn echo_client_recv( rx: Arc<UdpSocket>) ->std::io::Result<()> {
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
    
        emetrics::track_client_rx();

        if recv_time >= send_time {

            let delta = recv_time - send_time;
            let mut dtime: f64 = delta as f64;

            //convert to usec
            dtime = dtime/1000.0;

            #[cfg(test)]
            println!("{:?} detlta(us)", dtime);

            emetrics::track_latence(dtime);
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
        emetrics::track_client_tx();

        #[cfg(test)]{
            assert_eq!(len, mem::size_of::<u128>());
            println!("{:?} bytes sent", len);
        }
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

pub async fn run_latency_test(args: & Arc<eargs::Cli>) ->std::io::Result<()>{

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

    Ok(())
}
