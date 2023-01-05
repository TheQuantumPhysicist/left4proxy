use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::select;
use tokio::task;

#[cfg(test)]
mod test;

async fn do_tunnel(mut incoming: TcpStream, mut outgoing: TcpStream) {
    let mut buf1 = [0; 16384];
    let mut buf2 = [0; 16384];

    loop {
        select! {
            n = incoming.read(&mut buf1) => {
                // incoming has data available to be read
                let n = match n {
                    Ok(n) => n,
                    Err(e) => {
                        log::error!("Error reading from incoming: {}", e);
                        break;
                    }
                };

                if n == 0 {
                    log::debug!("Incoming has zero bytes. Closing connection"); // TODO: remove
                    break;
                } else {
                    log::debug!("Incoming has {n} bytes"); // TODO: remove
                }

                // Write the data from incoming to outgoing
                match outgoing.write_all(&buf1[0..n]).await {
                    Ok(()) => {},
                    Err(e) => {
                        log::error!("Error writing to outgoing: {}", e);
                        break;
                    }
                }
            },
            n = outgoing.read(&mut buf2) => {
                // outgoing has data available to be read
                let n = match n {
                    Ok(n) => n,
                    Err(e) => {
                        log::error!("Error reading from outgoing: {}", e);
                        break;
                    }
                };
                if n == 0 {
                    log::debug!("Outgoing has zero bytes. Closing connection"); // TODO: remove
                    break;
                } else {
                    log::debug!("Outgoing has {n} bytes"); // TODO: remove
                }

                // Write the data from outgoing to incoming
                match incoming.write_all(&buf2[0..n]).await {
                    Ok(()) => {},
                    Err(e) => {
                        log::error!("Error writing to incoming: {}", e);
                        break;
                    }
                }
            },
        }
    }
}

async fn handle_connection(incoming_stream: TcpStream, destinations: Arc<Vec<String>>) {
    for op in destinations.as_ref() {
        log::info!("Trying destination: {op}");
        let outgoing_stream = match TcpStream::connect(op).await {
            Ok(stream) => stream,
            Err(e) => {
                log::error!("Destination failed {op}: {e}");
                continue;
            }
        };
        log::info!("Connected to destination: {op}");

        do_tunnel(incoming_stream, outgoing_stream).await;
        return;
    }

    log::error!("All destinations failed: {destinations:?}");
}

fn parse_address<S: AsRef<str>>(addr_str: &S) -> Result<SocketAddr, Box<dyn std::error::Error>> {
    let addr_str = addr_str.as_ref();
    let addr = SocketAddr::from_str(addr_str).map_err(|e| {
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("Parse error of address: {addr_str} ; with error: {e}"),
        ))
    })?;
    Ok(addr)
}

fn validate_addresses(
    source: &String,
    destinations: &Vec<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    parse_address(source)?;
    for d in destinations {
        parse_address(d)?;
    }

    Ok(())
}

fn parse_args() -> Result<(String, Vec<String>), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 3 {
        eprintln!("Help:");
        eprintln!("http_forker <source:port> <destination1:port> <destination2:port> ...");
        eprintln!("    with at least one destination");
        eprintln!("destinations are attempted in order");
        eprintln!();

        return Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("Invalid call with not enough arguments: {}", args.join(" ")),
        )));
    }

    let source_address_str = args[1].clone();
    let destinations_strs = args[2..].to_vec();

    validate_addresses(&source_address_str, &destinations_strs)?;

    Ok((source_address_str, destinations_strs))
}

/// For the given TcpListener, any data that arrives, will be delivered to the given destination,
/// whichever is first available through a successful tcp connection, in order
async fn one_shot_proxy(listener: &TcpListener, destinations_strs: Arc<Vec<String>>) {
    let (stream, address) = match listener.accept().await {
        Ok(stream) => stream,
        Err(e) => {
            log::warn!("Accept failed: {e}");
            return;
        }
    };
    log::info!("Received connection from {address}");

    task::spawn(async {
        handle_connection(stream, destinations_strs).await;
    });
}

async fn start<S: AsRef<str>>(
    source_address_str: S,
    destinations_strs: Arc<Vec<String>>,
    bind_confirming_channel: Option<tokio::sync::oneshot::Sender<SocketAddr>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind(source_address_str.as_ref()).await?;

    let socket_bind_addr = listener
        .local_addr()
        .expect("Failed to retrieve socket address");

    if let Some(ch) = bind_confirming_channel {
        ch.send(socket_bind_addr)
            .expect("Oneshot: failed in sending binding result failed");
    }

    log::info!("Program started...");

    loop {
        let destinations_strs = Arc::clone(&destinations_strs);

        one_shot_proxy(&listener, destinations_strs).await
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (source_address_str, destinations_strs) = parse_args()?;

    let destinations_strs = Arc::new(destinations_strs);

    start(source_address_str, destinations_strs, None).await?;

    Ok(())
}
