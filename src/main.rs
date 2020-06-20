use std::net;
use std::env;
use std::net::{SocketAddr, UdpSocket};
use std::sync::mpsc;
use std::thread;

use crate::gtp_v1::GtpV1;
use std::borrow::Borrow;

mod gtp_v1;

/// UDP receiver thread
///
fn receiver_thread(address: SocketAddr, callback: &dyn Fn(&[u8], &UdpSocket) -> (), send_socket: &UdpSocket) {
    let receiver_socket = net::UdpSocket::bind(address).expect("Unable to open udp receive socket!");
    println!("Thread started!");

    loop {
        // receive loop
        let mut buf = [0; 2000];
        let (amt, _src) = match receiver_socket.recv_from(&mut buf) {
            Ok(res) => {res},
            Err(e) => {continue}
        };

        let buf = &mut buf[..amt];

        // send to queue
        callback(buf, send_socket);
    }
}

fn udp_callback(data: &[u8], socket: &UdpSocket) -> () {
    let mut packet = GtpV1::init(data.to_vec());
    socket.send(packet.serialize().as_ref());
}

fn gtp_callback(data: &[u8], socket: &UdpSocket) -> () {
    println!("GTP Callback");
    let mut packet = GtpV1::from_gtp(data);
    socket.send(packet.serialize().as_ref());
}

fn main() -> std::io::Result<()> {

    // parse args
    let args: Vec<String> = env::args().collect();
    assert!(args.len() >= (3 + 1), "Missing argument. Required <gtp target> <listener> <uplink target>");

    // --- Senders
    // address which producer gtp packets are send to
    let gtp_target = args.get(1).unwrap().parse::<SocketAddr>().unwrap();

    // address which, data of received gtp packets send to
    let uplink_udp_send_address = args.get(3).unwrap().parse::<SocketAddr>().unwrap();

    // --- Listeners
    // address which incoming udp packets are listened from
    let udp_listener_address = args.get(2).unwrap().parse::<SocketAddr>().unwrap();

    // address of incoming gtp packets listened from
    let gtp_listen_address = SocketAddr::new(
        udp_listener_address.ip(),
        2152
    );

    println!("{:?}",gtp_target);
    println!("{:?}",uplink_udp_send_address);
    println!("{:?}",udp_listener_address);
    println!("{:?}",gtp_listen_address);

    let gtp_sender = net::UdpSocket::bind(gtp_target).expect("Unable to open gtp sender socket!");
    gtp_sender.connect(gtp_target);
    let udp_sender= net::UdpSocket::bind(uplink_udp_send_address).expect("Unable to open udp sender socket!");
    udp_sender.connect(uplink_udp_send_address);

    // start receiver threads
    let udp_thread = thread::spawn(move || receiver_thread(udp_listener_address, &udp_callback, &gtp_sender));
    let gtp_thread = thread::spawn(move || receiver_thread(gtp_listen_address, &gtp_callback, &udp_sender));
    udp_thread.join();
    gtp_thread.join();

    Ok(())

}
