use std::time::Duration;

use tokio::time::timeout;
use zenoh_grpc_client_rs::{
    ConnectAddr, DeclareQueryableArgs, GrpcSession, QueryReplyArgs, SessionGetArgs,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let queryable_session = GrpcSession::connect(ConnectAddr::Tcp("127.0.0.1:7335".into())).await?;
    let getter_session = GrpcSession::connect(ConnectAddr::Tcp("127.0.0.1:7335".into())).await?;

    let queryable = queryable_session
        .declare_queryable(DeclareQueryableArgs {
            key_expr: "demo/query/**".into(),
            ..Default::default()
        }, None)
        .await?;

    tokio::spawn(async move {
        if let Ok(event) = queryable.receiver()?.recv_async().await {
            if let Some(query) = event.query {
                let _ = queryable
                    .reply(QueryReplyArgs {
                        query_id: query.query_id,
                        key_expr: "demo/query/value".into(),
                        payload: b"reply from rust".to_vec(),
                        ..Default::default()
                    })
                    .await;
            }
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
