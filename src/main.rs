extern crate serde;

use serde::{Serialize, Deserialize};
use std::{io::BufReader, thread::JoinHandle, net};
use std::env;
use std::net::{SocketAddr, IpAddr};
use std::panic;
use std::process;
use std::fs;
use std::collections::HashMap;

use crate::gtp_v1::GtpV1;
use std::thread;

mod gtp_v1;

struct UdpReceiverPayload {
    teid: u32,
    send_socket: net::UdpSocket,
    receive_address: SocketAddr
}

struct GtpReceiverPayload {
    teid_map: HashMap<u32, SenderReceiverPortPair>,
    receive_address: SocketAddr
}

#[derive(Serialize, Deserialize, Debug)]
struct HostPortPair {
    host: String,
    port: String
}

#[derive(Serialize, Deserialize, Debug)]
struct ReceiverSenderPair {
    receiver: HostPortPair,
    sender: HostPortPair
}

#[derive(Serialize, Deserialize, Debug)]
struct TeidConnectionPair {
    teid: u32,
    connection: ReceiverSenderPair
}

/// Config object
#[derive(Serialize, Deserialize, Debug)]
struct ConfigRoot {
    gtp: ReceiverSenderPair,
    tunnels: Vec<TeidConnectionPair>
}

struct SenderReceiverPortPair {
    teid: u32,
    sender: SocketAddr
}

/// Exit whole process on thread panic
///
fn set_exit_hook() {
    let orig_hook = std::panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        orig_hook(panic_info);
        process::exit(-1);
    }));
}

fn gtp_receiver_thread(payload: GtpReceiverPayload) {
    let receiver_socket = net::UdpSocket::bind(payload.receive_address).expect("Unable to bind to udp receive socket!");
    let sender_socket = net::UdpSocket::bind("0.0.0.0:0").expect("Unable to bind to sender socket");

    println!("GTP Receiver thread started for: {}:{}", payload.receive_address.ip(), payload.receive_address.port());
    
    loop {
        // receive loop
        let mut buf = [0; 2000];
        let (amt, _src) = match receiver_socket.recv_from(&mut buf) {
            Ok(res) => {res},
            Err(_e) => {continue}
        };

        let buf = &mut buf[..amt];

        // data receiver, handle
        let packet = GtpV1::from_gtp(buf);
        let addr = match payload.teid_map.get(&packet.get_teid()) {
            Some(a) => {a},
            None => {println!("Received message from undefined tunnel. Teid: {}", packet.get_teid()); continue;},
        };

        match sender_socket.send_to(packet.get_data(), addr.sender) {
            Ok(_) => {},
            Err(e) => {println!("Unable to send gtp data to udp socket: {:?}", e)},
        };
        
    }

}

fn udp_receiver_thread(payload: UdpReceiverPayload) {
    let receiver_socket = net::UdpSocket::bind(payload.receive_address).expect("Unable to bind to udp receive socket!");

    println!("UDP Receiver thread started for: {}:{}", payload.receive_address.ip(), payload.receive_address.port());

    loop {
        // receive loop
        let mut buf = [0; 2000];
        let (amt, _src) = match receiver_socket.recv_from(&mut buf) {
            Ok(res) => {res},
            Err(_e) => {continue}
        };

        let buf = &mut buf[..amt];

        // data receiver, handle
        let mut packet = GtpV1::init(buf.to_vec(), payload.teid);
        match payload.send_socket.send(packet.serialize().as_ref()) {
            Ok(_) => {},
            Err(e) => {println!("Unable to send gtp downlink packet, {:?}", e)},
        };

    }
}

fn main() -> std::io::Result<()> {
    set_exit_hook();

    // parse args
    let args: Vec<String> = env::args().collect();
    assert!(args.len() >= (1 + 1), "Missing argument. Required <config path>");

    // Read configuration from json
    let file = fs::File::open(args.get(1).unwrap())?;
    let reader = BufReader::new(file);
    let config: ConfigRoot = serde_json::from_reader(reader)?;

    let gtp_sender = SocketAddr::new(config.gtp.sender.host.parse::<IpAddr>().unwrap(), config.gtp.sender.port.parse::<u16>().unwrap());
    let gtp_receiver = SocketAddr::new(config.gtp.receiver.host.parse::<IpAddr>().unwrap(), config.gtp.receiver.port.parse::<u16>().unwrap());
    let gtp_send_socket = net::UdpSocket::bind("0.0.0.0:0").expect("Unable to bind to sender socket");
    gtp_send_socket.connect(gtp_sender).expect("Unable to connect to GTP sender socket.");

    let mut tunnels: HashMap<u32, SenderReceiverPortPair> = HashMap::new();
    let mut receiver_tunnels: HashMap<u32, SocketAddr> = HashMap::new();

    println!("GTP sender: {}:{}", gtp_sender.ip(), gtp_sender.port());
    println!("GTP receiver: {}:{}", gtp_receiver.ip(), gtp_receiver.port());
    println!("Tunnels:\n------");

    for (_index, tunnel) in config.tunnels.iter().enumerate() {
        let udp_receiver = SocketAddr::new(
            tunnel.connection.receiver.host.parse::<IpAddr>().unwrap(),
            tunnel.connection.receiver.port.parse::<u16>().unwrap()
        ); 
        let udp_sender = SocketAddr::new(
            tunnel.connection.sender.host.parse::<IpAddr>().unwrap(),
            tunnel.connection.sender.port.parse::<u16>().unwrap()
        );

        let pair = SenderReceiverPortPair{
            teid: tunnel.teid,
            sender: udp_sender
        };

        tunnels.insert(tunnel.teid, pair);  // move to map
        let pair = tunnels.get(&tunnel.teid).unwrap();  // get a ref to print info

        receiver_tunnels.insert(tunnel.teid, udp_receiver);
        let receiver_pair = receiver_tunnels.get(&tunnel.teid).unwrap();

        println!("{}- udp receive->{}:{}, udp send->{}:{}", 
            pair.teid,
            receiver_pair.ip(), 
            receiver_pair.port(), 
            pair.sender.ip(), 
            pair.sender.port()
        );
    }
    println!("-----");


    // start gtp receiver thread
    let gtp_thread = thread::spawn(move || gtp_receiver_thread(GtpReceiverPayload{
        teid_map: tunnels,
        receive_address: gtp_receiver
    }));

    // start udp receiver threads
    let threads: Vec<JoinHandle<_>> = receiver_tunnels.iter().map(|(&teid, &addr)| {
        let new_socket = gtp_send_socket.try_clone().unwrap();
        thread::spawn(move || udp_receiver_thread(UdpReceiverPayload{
            teid: teid.to_owned(),
            send_socket: new_socket,
            receive_address: addr.to_owned()
        }))
    }).collect();

    // join everything
    gtp_thread.join().unwrap();

    Ok(())

}
