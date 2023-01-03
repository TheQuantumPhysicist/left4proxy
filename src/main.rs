use async_std::io;
use async_std::net::{TcpListener, TcpStream};

const POSSIBLE_DESTINATIONS: [&str; 3] = ["127.0.0.1:55880", "10.10.0.11:55880", "127.0.0.1:8880"];

async fn do_tunnel(mut incoming: TcpStream, mut outgoing: TcpStream) {
    io::copy(&mut incoming, &mut outgoing)
        .await
        .unwrap_or_else(|e| {
            eprintln!("Copy from incoming to outgoing failed {e}");
            0
        });
    io::copy(&mut outgoing, &mut incoming)
        .await
        .unwrap_or_else(|e| {
            eprintln!("Copy from outgoing to incoming failed {e}");
            0
        });
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
