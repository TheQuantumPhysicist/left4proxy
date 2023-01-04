use async_std::{
    io::{ReadExt, WriteExt},
    net::ToSocketAddrs,
};

use super::*;

#[test]
fn check_valid_ipv4_addresses() {
    let addr = parse_address(&"127.0.0.1:123").unwrap();
    assert!(addr.is_ipv4());
    assert_eq!(addr.ip().to_string(), "127.0.0.1");
    assert_eq!(addr.port(), 123);

    let addr = parse_address(&"1.2.3.4:555").unwrap();
    assert!(addr.is_ipv4());
    assert_eq!(addr.ip().to_string(), "1.2.3.4");
    assert_eq!(addr.port(), 555);
}

#[test]
fn check_invalid_ipv4_addresses() {
    let _ = parse_address(&"256.0.0.1:123").unwrap_err();
    let _ = parse_address(&"127.0.0.1.9:123").unwrap_err();
    let _ = parse_address(&"127.0.0.1:66666").unwrap_err();
    let _ = parse_address(&"127.0.0:123").unwrap_err();
    let _ = parse_address(&"127.0.0.1").unwrap_err();
}

#[test]
fn check_valid_ipv6_addresses() {
    let addr = parse_address(&"[2345:0425:2CA1:0000:0000:0567:5673:23b5]:1245").unwrap();
    assert!(addr.is_ipv6());
    assert_eq!(
        addr.ip().to_string().to_uppercase(),
        "2345:425:2CA1::567:5673:23B5"
    );
    assert_eq!(addr.port(), 1245);

    let addr = parse_address(&"[::1]:2255").unwrap();
    assert!(addr.is_ipv6());
    assert_eq!(addr.ip().to_string(), "::1");
    assert_eq!(addr.port(), 2255);
}

#[test]
fn check_invalid_ipv6_addresses() {
    let _ = parse_address(&"256.0.0.1:123").unwrap_err();
    let _ = parse_address(&"127.0.0.1.9:123").unwrap_err();
    let _ = parse_address(&"127.0.0.1:66666").unwrap_err();
    let _ = parse_address(&"127.0.0:123").unwrap_err();
    let _ = parse_address(&"127.0.0.1").unwrap_err();
}

async fn prepare_destination_end(response_data: Vec<u8>) -> SocketAddr {
    let destination_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = destination_listener
        .local_addr()
        .unwrap()
        .to_socket_addrs()
        .await
        .unwrap()
        .into_iter()
        .collect::<Vec<SocketAddr>>()[0];

    let addr_clone = addr.clone();

    task::spawn(async move {
        let addr_clone = addr_clone;
        let destination_listener = destination_listener;
        let response_data = response_data;

        let (mut incoming_stream, _incoming_addr) = destination_listener.accept().await.unwrap();
        println!("Accepted connection in destination {addr_clone}");

        let read_buf = {
            let mut read_buf = Vec::new();
            loop {
                incoming_stream.read_to_end(&mut read_buf).await.unwrap();
                println!("Received bytes: {}", read_buf.len());

                if !read_buf.is_empty() {
                    break;
                }
            }
            read_buf
        };
        println!("Received: {:?}", read_buf);
        incoming_stream.write_all(&response_data);
        incoming_stream.write_all(&read_buf);
    });

    addr
}

#[async_std::test]
async fn connection_proxy() {
    // let source_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    // let source_listener_addr = source_listener
    //     .local_addr()
    //     .unwrap()
    //     .to_socket_addrs()
    //     .await
    //     .unwrap()
    //     .into_iter()
    //     .collect::<Vec<SocketAddr>>()[0];

    let destinations_count = 4;

    let mut destinations_addrs = Vec::new();
    for _index in 0..destinations_count {
        let addr = prepare_destination_end(b"abcd".to_vec()).await;

        destinations_addrs.push(addr);
    }

    let destinations_addrs_strs = destinations_addrs
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<String>>();

    println!("Test destinations: {destinations_addrs_strs:?}");
    let destinations_strs = Arc::new(destinations_addrs_strs.clone());

    task::spawn(async {
        // one_shot_proxy(&source_listener, Arc::new(destinations_addrs_strs)).await;
        start("127.0.0.1:53535", destinations_strs).await.unwrap();
    });

    let mut source_writer = TcpStream::connect("127.0.0.1:53535").await.unwrap();

    let data_to_send = vec![1, 5, 7];
    source_writer.write(&mut &data_to_send).await.unwrap();
    let mut read_buf = Vec::new();

    std::thread::sleep(std::time::Duration::from_secs(5));

    source_writer.read_to_end(&mut read_buf).await.unwrap();
    println!("{:?}", read_buf);
}
