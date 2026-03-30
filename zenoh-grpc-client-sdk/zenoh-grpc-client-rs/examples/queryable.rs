use std::time::Duration;

use tokio::time::timeout;
use zenoh_grpc_client_rs::{ConnectAddr, DeclareQueryableArgs, GrpcSession, SessionGetArgs};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let queryable_session = GrpcSession::connect(ConnectAddr::Tcp("127.0.0.1:7335".into())).await?;
    let getter_session = GrpcSession::connect(ConnectAddr::Tcp("127.0.0.1:7335".into())).await?;

    let queryable = queryable_session
        .declare_queryable(
            DeclareQueryableArgs {
                key_expr: "demo/query/**".into(),
                ..Default::default()
            },
            None,
        )
        .await?;

    tokio::spawn(async move {
        if let Ok(query) = queryable.recv_async().await {
            let _ = query
                .reply(
                    "demo/query/value",
                    b"reply from rust".to_vec(),
                    "text/plain",
                    Vec::new(),
                    String::new(),
                )
                .await;
            let _ = query.finish().await;
        }
        Ok::<(), zenoh_grpc_client_rs::Error>(())
    });

    let replies = getter_session
        .get(SessionGetArgs {
            selector: "demo/query/value".into(),
            ..Default::default()
        })
        .await?;
    let reply = timeout(Duration::from_secs(5), replies.recv_async()).await??;
    println!("{reply:?}");
    Ok(())
}
