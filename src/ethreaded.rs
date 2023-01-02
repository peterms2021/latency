// use thread bound vs tasks for measuring app latency


use rand::{thread_rng, Rng};
use std::time::{SystemTime, UNIX_EPOCH};
use std::sync::Arc;
use std::{u128 };
use std::net::{ UdpSocket, ToSocketAddrs};
use std::mem;
use std::{thread, time};

use crate::eargs;

fn induce_delay() {
    let mut rng = thread_rng();
    let delay_time: u32 = rng.gen_range(1..20);
    let sleep_ms = time::Duration::from_millis(delay_time as u64);

    //TEST_LATENCY_COLLECTOR.observe(delay_time as f64);
    thread::sleep(sleep_ms);
}

pub fn echo_server(port: u16)-> std::io::Result<()> {
    
    let socket = UdpSocket::bind("0.0.0.0:".to_string() + &port.to_string())?;
    println!("\nstart server on port {} \n", port);

    loop {
        let mut buf = vec![0u8; 1000];
        let (amt, src) = socket.recv_from(&mut buf)?;

#[cfg(test)]
        println!("bytes: {:?} From: {:?}, buf: {:?}",amt, src, &buf[..amt]);

        socket.send_to(&buf[..amt], src)?;
        
        main::track_server_echos();
    }
}

fn echo_client_recv( rx: Arc<UdpSocket>) ->std::io::Result<()> {
    println!(" echo_client_recv");

    let mut buf = vec![0u8; 1000];
    loop {

        //let n = rx.recv(&mut buf).await?;
        let (n, addr) = rx.recv_from(&mut buf)?;

#[cfg(test)]
        {
            println!("{:?} recv from {:?}", n,addr);
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
    
        emetrics::track_client_rx();

        if recv_time >= send_time {

            let mut delta = recv_time - send_time;
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

fn echo_client_sender(tx: Arc<UdpSocket>, interval: u32)->std::io::Result<()> {
    let sleep_ms = time::Duration::from_millis(interval as u64);
    loop {

        thread::sleep(sleep_ms);

        let time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        let bytes = time.to_be_bytes();
        let len = tx.send(&bytes)?;
        emetrics::track_client_tx();

        #[cfg(test)]{
            assert_eq!(len, mem::size_of::<u128>());
            println!("{:?} bytes sent", len);
        }
    }
}

fn echo_client(addr: String, port: u16, interval: u32)->std::io::Result<()>{

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
    thread::spawn(move ||  {
        echo_client_sender(tx,interval).expect("echo client sender failed");
    });

    thread::spawn( move || {
        echo_client_recv(rx).expect("client recv tasks failed");
    });

    //spawn the sender
    Ok(())
}

pub fn run_latency_test(args: & Arc<eargs::Cli>) ->std::io::Result<()>{

    if args.emode == true {
        let x = Arc::clone(&args);
        let svr = thread::spawn(  move || {
            echo_server(x.port);
        });
    }else{
        //check for valid address
        let x = Arc::clone(&args);
        let cli = thread::spawn(move  || {
            echo_client( x.server_addr.clone(), x.port, x.interval);
        });
    }

    Ok(())
}
