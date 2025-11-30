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
    request_shutdown_signal: tokio::sync::mpsc::UnboundedReceiver<()>,
    end_reached_signal: tokio::sync::mpsc::UnboundedSender<()>,
}

/// This function launches a destination, and prepares it to receive `expected_data`,
/// and it replies with is `response_prefix`, followed by `expected_data`
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

        // we first write the prefix in the response, which is part of the algorithm of the test
        incoming_stream.write_all(&response_prefix).await.unwrap();

        let mut full_incoming_buffer = Vec::<u8>::new();

        let mut incoming_buffer_chunk = [0; 16384];

        loop {
            select! {
                n = incoming_stream.read(&mut incoming_buffer_chunk) => {
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

                    assert!(expected_data.starts_with(&full_incoming_buffer));
                    assert!(full_incoming_buffer.len() <= expected_data.len());

                    // Write the data from incoming to outgoing
                    println!("Writing incoming data of destination {addr_clone} back to stream ({n} bytes)");
                    match incoming_stream.write_all(&incoming_buffer_chunk[0..n]).await {
                        Ok(()) => {},
                        Err(e) => {
                            eprintln!("Error writing to outgoing: {}", e);
                            break;
                        }
                    }
                },
                _ = signals.request_shutdown_signal.recv() => {
                    // when shutting down, we expect that all the data has been delivered already,
                    // so we ensure it's equal to expected_data
                    assert_eq!(full_incoming_buffer, expected_data);
                    println!("Buffer equality for destination {addr_clone} passed");
                    break;
                }

            }
        }

        // actively destroy the destination
        drop(incoming_stream);
        drop(destination_listener);

        signals.end_reached_signal.send(()).unwrap();
    });

    addr
}

fn random_size(max_size: usize) -> usize {
    rand::random_range(0..max_size)
}

fn random_bytes(max_size: usize) -> Vec<u8> {
    (0..random_size(max_size))
        .map(|_| rand::random::<u8>())
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 10)]
async fn connection_proxy() {
    let destinations_count = rand::random_range(0..100);
    println!("Test destinations count: {}", destinations_count);

    let max_size = 1 << 20;

    let response_prefixes = (0..destinations_count)
        .into_iter()
        .map(|_| random_bytes(max_size))
        .collect::<Vec<_>>();

    let expected_data_for_dests = (0..destinations_count)
        .into_iter()
        .map(|_| random_bytes(max_size))
        .collect::<Vec<_>>();

    //////////////////////////////////////////////////
    // Create signals to communicate with destinations
    let (
        destinations_request_shutdown_signal_senders,
        mut destinations_request_shutdown_signal_receivers,
    ): (Vec<_>, Vec<_>) = (0..destinations_count)
        .into_iter()
        .map(|_idx| tokio::sync::mpsc::unbounded_channel())
        .unzip();

    let (mut end_reached_signal_senders, mut end_reached_signal_receivers): (Vec<_>, Vec<_>) = (0
        ..destinations_count)
        .into_iter()
        .map(|_idx| tokio::sync::mpsc::unbounded_channel())
        .unzip();

    // populate the signals
    let mut destinations_signals = Vec::new();
    for _ in 0..destinations_count {
        let s = DestinationSignals {
            request_shutdown_signal: destinations_request_shutdown_signal_receivers
                .pop()
                .unwrap(),
            end_reached_signal: end_reached_signal_senders.pop().unwrap(),
        };
        destinations_signals.push(s);
    }
    destinations_signals.reverse(); // since we filled them in reverse order using pop()

    //////////////////////////////////////////////////

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

    // create the destinations and retrieve their addresses
    let destinations_addrs_strs = {
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

        destinations_addrs_strs
    };

    println!("Test destinations: {destinations_addrs_strs:?}");
    let destinations_strs = Arc::new(destinations_addrs_strs.clone());

    // Start the proxy application on a specified port
    let socket_bind_addr = {
        let (start_bind_tx, start_bind_rx) = tokio::sync::oneshot::channel();

        task::spawn(async {
            start("127.0.0.1:0", destinations_strs, Some(start_bind_tx))
                .await
                .unwrap();
        });

        start_bind_rx.await.unwrap()
    };

    // In every iteration, we connect to the proxy, then expect the data to arrive to the first available destination,
    // then we close the destination (drop it), reconnect, and we expect to move to the next one
    for idx in 0..destinations_count {
        let mut source_writer = TcpStream::connect(socket_bind_addr).await.unwrap();

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

        destinations_request_shutdown_signal_senders[idx]
            .send(())
            .unwrap();

        end_reached_signal_receivers[idx].recv().await.unwrap();

        drop(source_writer);
    }
}
