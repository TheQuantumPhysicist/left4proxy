use std::net::ToSocketAddrs;

use tokio::io::{AsyncReadExt, AsyncWriteExt};

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

struct DestinationSignals {
    request_shutdown_signal: tokio::sync::oneshot::Receiver<()>,
}

async fn prepare_destination_end(
    response_prefix: Vec<u8>,
    expected_data: Vec<u8>,
    mut signals: DestinationSignals,
) -> SocketAddr {
    let destination_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = destination_listener
        .local_addr()
        .unwrap()
        .to_socket_addrs()
        .unwrap()
        .into_iter()
        .collect::<Vec<SocketAddr>>()[0];

    let addr_clone = addr.clone();

    task::spawn(async move {
        let addr_clone = addr_clone;
        let destination_listener = destination_listener;
        let response_prefix = response_prefix;
        let expected_data = expected_data;

        let (mut incoming_stream, _incoming_addr) = destination_listener.accept().await.unwrap();
        println!("Accepted connection in destination {addr_clone}");

        // we first write the prefix in the response
        incoming_stream.write_all(&response_prefix).await.unwrap();

        let mut full_incoming_buffer = Vec::<u8>::new();

        let mut incoming_buffer_chunk = [0; 16384];

        loop {
            select! {
                n = incoming_stream.read(&mut incoming_buffer_chunk) => {
                    // incoming has data available to be read
                    let n = match n {
                        Ok(n) => n,
                        Err(e) => {
                            eprintln!("Error reading from incoming: {}", e);
                            break;
                        }
                    };

                    if n == 0 {
                        println!("Destination {addr_clone}: Incoming has zero bytes. Closing connection");
                        break;
                    } else {
                        println!("Destination {addr_clone}: Incoming has {n} bytes");
                    }

                    full_incoming_buffer.extend(&incoming_buffer_chunk[0..n]);

                    assert!(full_incoming_buffer.starts_with(&expected_data));
                    assert!(full_incoming_buffer.len() <= expected_data.len());

                    // Write the data from incoming to outgoing
                    match incoming_stream.write_all(&incoming_buffer_chunk[0..n]).await {
                        Ok(()) => {},
                        Err(e) => {
                            eprintln!("Error writing to outgoing: {}", e);
                            break;
                        }
                    }
                },
                _ = &mut signals.request_shutdown_signal => {
                    // TODO: better than shutdown and check, we send the data we wanna check in the buffer
                    // after everything is done, ensure that the data we received represents the full expected data
                    assert_eq!(full_incoming_buffer, expected_data);
                    println!("Buffer equality for destination {addr_clone} passed");
                    break;
                }

            }
        }
    });

    addr
}

#[tokio::test]
async fn connection_proxy() {
    let destinations_count = 4;

    let response_prefixes = [
        b"abcd".to_vec(),
        b"ggg".to_vec(),
        b"shas".to_vec(),
        b"bassX".to_vec(),
    ];

    let expected_data_for_dests = [
        b"sdsdsddwwd".to_vec(),
        b"sgfsgsgs".to_vec(),
        b"fwfwfwfw".to_vec(),
        b"sdfsfswegegegeg".to_vec(),
    ];

    let (
        destinations_request_shutdown_signal_senders,
        destinations_request_shutdown_signal_receivers,
    ): (Vec<_>, Vec<_>) = (0..destinations_count)
        .into_iter()
        .map(|_idx| tokio::sync::oneshot::channel())
        .unzip();

    let destinations_signals = destinations_request_shutdown_signal_receivers
        .into_iter()
        .map(|d| DestinationSignals {
            request_shutdown_signal: d,
        })
        .collect::<Vec<_>>();

    let prefixes_expecteddata_signals: Vec<(Vec<u8>, Vec<u8>, DestinationSignals)> =
        response_prefixes
            .clone()
            .into_iter()
            .zip(
                expected_data_for_dests
                    .clone()
                    .into_iter()
                    .zip(destinations_signals.into_iter()),
            )
            .map(|(a, (b, c))| (a, b, c))
            .collect();

    let destinations_initializers = prefixes_expecteddata_signals
        .into_iter()
        .map(
            |(response_prefixes, expected_data_for_dest, destinations_signals)| {
                prepare_destination_end(
                    response_prefixes,
                    expected_data_for_dest,
                    destinations_signals,
                )
            },
        )
        .collect::<Vec<_>>();

    let mut destinations_addrs = Vec::new();
    for dest in destinations_initializers.into_iter() {
        let addr = dest.await;

        destinations_addrs.push(addr);
    }

    let destinations_addrs_strs = destinations_addrs
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<String>>();

    println!("Test destinations: {destinations_addrs_strs:?}");
    let destinations_strs = Arc::new(destinations_addrs_strs.clone());

    let (start_bind_tx, start_bind_rx) = tokio::sync::oneshot::channel();

    task::spawn(async {
        // one_shot_proxy(&source_listener, Arc::new(destinations_addrs_strs)).await;
        start("127.0.0.1:53535", destinations_strs, Some(start_bind_tx))
            .await
            .unwrap();
    });

    start_bind_rx.await.unwrap();

    let mut source_writer = TcpStream::connect("127.0.0.1:53535").await.unwrap();

    let idx = 0; // destination index TODO put in loop

    let data_to_send = expected_data_for_dests[idx].clone();

    source_writer.write_all(&data_to_send).await.unwrap();

    let expected_data = response_prefixes[idx]
        .clone()
        .into_iter()
        .chain(data_to_send.clone().into_iter())
        .collect::<Vec<u8>>();

    let mut read_result: Vec<u8> = Vec::new();
    read_result.resize(expected_data.len(), 0);
    source_writer.read_exact(&mut read_result).await.unwrap();

    assert_eq!(read_result, expected_data);

    destinations_request_shutdown_signal_senders
        .into_iter()
        .for_each(|s| {
            s.send(()).unwrap();
        });
}
