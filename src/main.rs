use std::net;
use std::env;
use std::net::{SocketAddr, UdpSocket};
use std::thread;

use crate::gtp_v1::GtpV1;
mod gtp_v1;

/// UDP receiver thread
///
fn receiver_thread(address: SocketAddr, callback: &dyn Fn(&[u8], &UdpSocket) -> (), send_address: SocketAddr) {
    let receiver_socket = net::UdpSocket::bind(address).expect("Unable to open udp receive socket!");
    let sender_socket = net::UdpSocket::bind("0.0.0.0:0").expect("Unable to open sender socket");
    sender_socket.connect(send_address).unwrap();

    println!("Thread started!");

    loop {
        // receive loop
        let mut buf = [0; 2000];
        let (amt, _src) = match receiver_socket.recv_from(&mut buf) {
            Ok(res) => {res},
            Err(_e) => {continue}
        };

        let buf = &mut buf[..amt];

        // send to queue
        callback(buf, &sender_socket);
    }
}

fn udp_callback(data: &[u8], socket: &UdpSocket) -> () {
    let mut packet = GtpV1::init(data.to_vec());
    match socket.send(packet.serialize().as_ref()) {
        Ok(_) => {},
        Err(e) => {println!("Unable to send, {:?}", e)}
    }
}

fn gtp_callback(data: &[u8], socket: &UdpSocket) -> () {
    let packet = GtpV1::from_gtp(data);
    match socket.send(packet.get_data()) {
        Ok(_) => {},
        Err(e) => {println!("Unable to send, {:?}", e)}
    }
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

    // start receiver threads
    let udp_thread = thread::spawn(move || receiver_thread(udp_listener_address, &udp_callback, gtp_target));
    let gtp_thread = thread::spawn(move || receiver_thread(gtp_listen_address, &gtp_callback, uplink_udp_send_address));
    udp_thread.join().unwrap();
    gtp_thread.join().unwrap();

    Ok(())

}
