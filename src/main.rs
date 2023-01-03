use async_std::net::{TcpListener, TcpStream};
use async_std::prelude::*;

const POSSIBLE_DESTINATIONS: [&str; 3] = ["127.0.0.1:55880", "10.10.0.11:55880", "127.0.0.1:8880"];

async fn do_tunnel(mut incoming: TcpStream, mut outgoing: TcpStream) {
    let mut buf = Vec::new();
    loop {
        match incoming.read_to_end(&mut buf).await {
            Ok(_) => (),
            Err(e) => {
                eprintln!("Buffer incoming read error: {e}");
                return;
            }
        }

        match outgoing.write_all(&buf).await {
            Ok(_) => (),
            Err(e) => {
                eprintln!("Buffer outgoing write error: {e}");
                return;
            }
        }
    }
}

async fn handle_connection(incoming_stream: TcpStream) {
    for op in POSSIBLE_DESTINATIONS {
        let outgoing_stream = match TcpStream::connect(op).await {
            Ok(stream) => stream,
            Err(e) => {
                eprintln!("Destination failed {op}: {e}");
                continue;
            }
        };

        do_tunnel(incoming_stream, outgoing_stream).await;
        return;
    }

    eprintln!("All destinations failed: {POSSIBLE_DESTINATIONS:?}");
}

#[async_std::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind("127.0.0.1:58888").await?;
    loop {
        let (stream, address) = match listener.accept().await {
            Ok(stream) => stream,
            Err(e) => {
                eprintln!("Accept failed: {e}");
                continue;
            }
        };
        println!("Received connection from {address}");
        handle_connection(stream).await;
    }
}