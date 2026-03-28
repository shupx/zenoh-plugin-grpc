use zenoh_grpc_client_rs::{ConnectAddr, GrpcSession};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let session = GrpcSession::connect(ConnectAddr::Tcp("127.0.0.1:7335".into())).await?;
    let info = session.info().await?;
    println!("Connected to Zenoh node {}", info.zid);
    Ok(())
}
