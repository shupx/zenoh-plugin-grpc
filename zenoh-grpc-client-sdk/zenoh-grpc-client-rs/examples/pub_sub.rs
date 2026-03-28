use std::time::Duration;

use tokio::time::timeout;
use zenoh_grpc_client_rs::{
    ConnectAddr, DeclarePublisherArgs, DeclareSubscriberArgs, GrpcSession, PublisherPutArgs,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let sub_session = GrpcSession::connect(ConnectAddr::Tcp("127.0.0.1:7335".into())).await?;
    let pub_session = GrpcSession::connect(ConnectAddr::Tcp("127.0.0.1:7335".into())).await?;

    let subscriber = sub_session
        .declare_subscriber(DeclareSubscriberArgs {
            key_expr: "demo/example/**".into(),
            ..Default::default()
        })
        .await?;
    let publisher = pub_session
        .declare_publisher(DeclarePublisherArgs {
            key_expr: "demo/example/value".into(),
            ..Default::default()
        })
        .await?;

    publisher
        .put(PublisherPutArgs {
            payload: b"hello from rust".to_vec(),
            ..Default::default()
        })
        .await?;

    let event = timeout(Duration::from_secs(5), subscriber.receiver().recv_async()).await??;
    if let Some(sample) = event.sample {
        println!(
            "received {} => {}",
            sample.key_expr,
            String::from_utf8_lossy(&sample.payload)
        );
    }

    Ok(())
}
