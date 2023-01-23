// use thread bound vs tasks for measuring app latency


#[cfg(dtest)]
use rand::{thread_rng, Rng};

#[cfg(test)]
use std::{mem };

use std::{thread, time, u128};

use std::net::{UdpSocket, ToSocketAddrs};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::eargs;
use crate::emetrics::Metrics;

pub fn run_latency_test(args: & Arc<eargs::Cli>, metrics: Arc<Metrics>) ->std::io::Result<()>{
    if args.emode == true {
        let x = Arc::clone(&args);
        let _svr = thread::spawn(  move || {
            echo_server(x.port, metrics).unwrap();
        });
    }else{
        //check for valid address
        let x = Arc::clone(&args);
        let _cli = thread::spawn(move  || {
            echo_client( x.server_addr.clone(), x.port, x.interval, metrics).unwrap();
        });
    }

    Ok(())
}


#[cfg(dtest)]
fn induce_delay() {
    let mut rng = thread_rng();
    let delay_time: u32 = rng.gen_range(1..20);
    let sleep_ms = time::Duration::from_millis(delay_time as u64);

    //TEST_LATENCY_COLLECTOR.observe(delay_time as f64);
    thread::sleep(sleep_ms);
}

pub fn echo_server(port: u16, metrics: Arc<Metrics>)-> std::io::Result<()> {
    
    let socket = UdpSocket::bind("0.0.0.0:".to_string() + &port.to_string())?;
    println!("\nStarted server thread on port {} \n", port);
    loop {
        let mut buf = vec![0u8; 1000];
        let (amt, src) = socket.recv_from(&mut buf)?;

#[cfg(test)]
        println!("bytes: {:?} From: {:?}, buf: {:?}",amt, src, &buf[..amt]);

        socket.send_to(&buf[..amt], src)?;
        
        metrics.track_server_echoes();
    }
}

fn echo_client_recv(rx: Arc<UdpSocket>, metrics: Arc<Metrics>) ->std::io::Result<()> {

    println!(" echo_client_recv thread");
    let mut buf = vec![0u8; 1000];
    loop {

        //let n = rx.recv(&mut buf).await?;
        let (n, _addr) = rx.recv_from(&mut buf)?;
        
#[cfg(test)]
        {
            println!("{:?} recv from {:?}", n,_addr);
            assert_eq!(n, mem::size_of::<u128>());
        }

        #[cfg(dtest)]
        induce_delay();

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

fn echo_client_sender(tx: Arc<UdpSocket>, interval: u32, metrics: Arc<Metrics>)->std::io::Result<()> {
    let sleep_ms = time::Duration::from_millis(interval as u64);
    loop {

        thread::sleep(sleep_ms);

        let time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        let bytes = time.to_be_bytes();
        let _len = tx.send(&bytes)?;
        metrics.track_client_tx();
        #[cfg(test)]{
            assert_eq!(_len, mem::size_of::<u128>());
            println!("{:?} bytes sent", _len);
        }
    }
}

fn echo_client(addr: String, port: u16, interval: u32, metrics: Arc<Metrics>)->std::io::Result<()>{

    let ad = addr + ":" + &port.to_string();
    let _remote_addr_iter= ad.to_socket_addrs().expect("Unable to resolve domain");

    println!("Remote addr {}",ad);

    // We use port 0 to let the operating system allocate an available port for us.
    let local_addr  = "0.0.0.0:0".to_string();

    let sock = UdpSocket::bind(local_addr)?;
    //let send_addr = addr + ":" + &port.to_string();
    sock.connect(ad)?;

    let rx = Arc::new(sock);
    let tx = rx.clone();    

    //spawn thread that will wait for the echos
    let send_metrics = metrics.clone();
    thread::spawn(move ||  {
        echo_client_sender(tx,interval, send_metrics).expect("echo client sender failed");
    });

    let rcv_metrics = metrics.clone();
    thread::spawn( move || {
        echo_client_recv(rx, rcv_metrics).expect("client recv tasks failed");
    });

    //spawn the sender
    Ok(())
}
