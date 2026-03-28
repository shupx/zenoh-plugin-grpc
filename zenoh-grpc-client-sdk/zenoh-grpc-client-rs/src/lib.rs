mod args;
mod queue;

use std::{path::PathBuf, sync::Arc};

use hyper_util::rt::TokioIo;
use thiserror::Error;
use tokio::net::UnixStream;
use tonic::transport::{Channel, Endpoint};
use tower::service_fn;
use uuid::Uuid;
use zenoh_grpc_proto::v1::{
    self as pb,
    publisher_service_client::PublisherServiceClient,
    querier_service_client::QuerierServiceClient,
    queryable_service_client::QueryableServiceClient,
    session_service_client::SessionServiceClient,
    subscriber_service_client::SubscriberServiceClient,
};

pub use args::*;
pub use queue::DropOldestReceiver;
use queue::{bounded_drop_oldest, DropOldestSender};

#[derive(Debug, Clone)]
pub enum ConnectAddr {
    Tcp(String),
    Unix(PathBuf),
}

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Transport(#[from] tonic::transport::Error),
    #[error(transparent)]
    Status(#[from] tonic::Status),
}

#[derive(Clone)]
struct Inner {
    channel: Channel,
    client_id: String,
}

#[derive(Clone)]
pub struct GrpcSession {
    inner: Arc<Inner>,
    write_tx: DropOldestSender<SessionCommand>,
}

#[derive(Clone)]
pub struct GrpcPublisher {
    session: GrpcSession,
    handle: u64,
    write_tx: DropOldestSender<PublisherCommand>,
}

#[derive(Clone)]
pub struct GrpcSubscriber {
    session: GrpcSession,
    handle: u64,
    rx: DropOldestReceiver<pb::SubscriberEvent>,
}

#[derive(Clone)]
pub struct GrpcQueryable {
    session: GrpcSession,
    handle: u64,
    rx: DropOldestReceiver<pb::QueryableEvent>,
    write_tx: DropOldestSender<QueryableCommand>,
}

#[derive(Clone)]
pub struct GrpcQuerier {
    session: GrpcSession,
    handle: u64,
}

#[derive(Debug, Clone)]
enum SessionCommand {
    Put(SessionPutArgs),
    Delete(SessionDeleteArgs),
}

#[derive(Debug, Clone)]
enum PublisherCommand {
    Put(PublisherPutArgs),
    Delete(PublisherDeleteArgs),
}

#[derive(Debug, Clone)]
enum QueryableCommand {
    Reply(QueryReplyArgs),
    ReplyErr(QueryReplyErrArgs),
    ReplyDelete(QueryReplyDeleteArgs),
}

const SEND_QUEUE_CAPACITY: usize = 256;
const RECV_QUEUE_CAPACITY: usize = 256;

async fn connect_channel(addr: &ConnectAddr) -> Result<Channel, tonic::transport::Error> {
    match addr {
        ConnectAddr::Tcp(addr) => Endpoint::from_shared(format!("http://{addr}"))?.connect().await,
        ConnectAddr::Unix(path) => Endpoint::try_from("http://[::]:50051")?
            .connect_with_connector(service_fn({
                let path = path.clone();
                move |_| {
                    let path = path.clone();
                    async move { UnixStream::connect(path).await.map(TokioIo::new) }
                }
            }))
            .await,
    }
}

impl GrpcSession {
    pub async fn connect(addr: ConnectAddr) -> Result<Self, Error> {
        let channel = connect_channel(&addr).await?;
        let inner = Arc::new(Inner {
            channel,
            client_id: Uuid::new_v4().to_string(),
        });
        let (write_tx, write_rx) = bounded_drop_oldest(SEND_QUEUE_CAPACITY);
        let session = Self {
            inner,
            write_tx,
        };
        session.spawn_session_worker(write_rx);
        Ok(session)
    }

    fn session_client(&self) -> SessionServiceClient<Channel> {
        SessionServiceClient::new(self.inner.channel.clone())
    }

    fn publisher_client(&self) -> PublisherServiceClient<Channel> {
        PublisherServiceClient::new(self.inner.channel.clone())
    }

    fn subscriber_client(&self) -> SubscriberServiceClient<Channel> {
        SubscriberServiceClient::new(self.inner.channel.clone())
    }

    fn queryable_client(&self) -> QueryableServiceClient<Channel> {
        QueryableServiceClient::new(self.inner.channel.clone())
    }

    fn querier_client(&self) -> QuerierServiceClient<Channel> {
        QuerierServiceClient::new(self.inner.channel.clone())
    }

    fn spawn_session_worker(&self, rx: DropOldestReceiver<SessionCommand>) {
        let session = self.clone();
        tokio::spawn(async move {
            while let Ok(command) = rx.recv_async().await {
                match command {
                    SessionCommand::Put(req) => {
                        let _ = session
                            .session_client()
                            .put(pb::SessionPutRequest {
                                client_id: session.inner.client_id.clone(),
                                key_expr: req.key_expr,
                                payload: req.payload,
                                encoding: req.encoding,
                                congestion_control: req.congestion_control,
                                priority: req.priority,
                                express: req.express,
                                attachment: req.attachment,
                                timestamp: req.timestamp,
                                allowed_destination: req.allowed_destination,
                            })
                            .await;
                        rx.processed_one();
                    }
                    SessionCommand::Delete(req) => {
                        let _ = session
                            .session_client()
                            .delete(pb::SessionDeleteRequest {
                                client_id: session.inner.client_id.clone(),
                                key_expr: req.key_expr,
                                congestion_control: req.congestion_control,
                                priority: req.priority,
                                express: req.express,
                                attachment: req.attachment,
                                timestamp: req.timestamp,
                                allowed_destination: req.allowed_destination,
                            })
                            .await;
                        rx.processed_one();
                    }
                }
            }
        });
    }

    pub fn client_id(&self) -> &str {
        &self.inner.client_id
    }

    pub fn send_dropped_count(&self) -> u64 {
        self.write_tx.dropped_count()
    }

    pub async fn info(&self) -> Result<pb::SessionInfoReply, Error> {
        Ok(self
            .session_client()
            .info(pb::SessionInfoRequest {
                client_id: self.inner.client_id.clone(),
            })
            .await?
            .into_inner())
    }

    pub async fn put(&self, req: SessionPutArgs) -> Result<(), Error> {
        self.write_tx
            .push(SessionCommand::Put(req))
            .map_err(|_| tonic::Status::unavailable("session send queue closed").into())
    }

    pub async fn delete(&self, req: SessionDeleteArgs) -> Result<(), Error> {
        self.write_tx
            .push(SessionCommand::Delete(req))
            .map_err(|_| tonic::Status::unavailable("session send queue closed").into())
    }

    pub async fn get(&self, req: SessionGetArgs) -> Result<DropOldestReceiver<pb::Reply>, Error> {
        let mut stream = self
            .session_client()
            .get(pb::SessionGetRequest {
                client_id: self.inner.client_id.clone(),
                selector: req.selector,
                target: req.target,
                consolidation: req.consolidation,
                timeout_ms: req.timeout_ms,
                payload: req.payload,
                encoding: req.encoding,
                attachment: req.attachment,
                allowed_destination: req.allowed_destination,
            })
            .await?
            .into_inner();
        let (tx, rx) = bounded_drop_oldest(RECV_QUEUE_CAPACITY);
        tokio::spawn(async move {
            while let Ok(Some(reply)) = stream.message().await {
                let _ = tx.push(reply);
            }
        });
        Ok(rx)
    }

    pub async fn cleanup(&self) -> Result<(), Error> {
        self.write_tx.wait_empty().await;
        self.session_client()
            .cleanup_client(pb::CleanupClientRequest {
                client_id: self.inner.client_id.clone(),
            })
            .await?;
        Ok(())
    }

    pub async fn declare_publisher(&self, req: DeclarePublisherArgs) -> Result<GrpcPublisher, Error> {
        let handle = self
            .publisher_client()
            .declare_publisher(pb::DeclarePublisherRequest {
                client_id: self.inner.client_id.clone(),
                key_expr: req.key_expr,
                encoding: req.encoding,
                congestion_control: req.congestion_control,
                priority: req.priority,
                express: req.express,
                reliability: req.reliability,
                allowed_destination: req.allowed_destination,
            })
            .await?
            .into_inner()
            .handle;
        Ok(GrpcPublisher {
            session: self.clone(),
            handle,
            write_tx: {
                let (tx, rx) = bounded_drop_oldest(SEND_QUEUE_CAPACITY);
                let publisher = GrpcPublisher {
                    session: self.clone(),
                    handle,
                    write_tx: tx.clone(),
                };
                publisher.spawn_worker(rx);
                tx
            },
        })
    }

    pub async fn declare_subscriber(&self, req: DeclareSubscriberArgs) -> Result<GrpcSubscriber, Error> {
        let handle = self
            .subscriber_client()
            .declare_subscriber(pb::DeclareSubscriberRequest {
                client_id: self.inner.client_id.clone(),
                key_expr: req.key_expr,
                allowed_origin: req.allowed_origin,
            })
            .await?
            .into_inner()
            .handle;
        let mut stream = self
            .subscriber_client()
            .events(pb::SubscriberEventsRequest {
                client_id: self.inner.client_id.clone(),
                handle,
            })
            .await?
            .into_inner();
        let (tx, rx) = bounded_drop_oldest(RECV_QUEUE_CAPACITY);
        tokio::spawn(async move {
            while let Ok(Some(event)) = stream.message().await {
                let _ = tx.push(event);
            }
        });
        Ok(GrpcSubscriber {
            session: self.clone(),
            handle,
            rx,
        })
    }

    pub async fn declare_queryable(&self, req: DeclareQueryableArgs) -> Result<GrpcQueryable, Error> {
        let handle = self
            .queryable_client()
            .declare_queryable(pb::DeclareQueryableRequest {
                client_id: self.inner.client_id.clone(),
                key_expr: req.key_expr,
                complete: req.complete,
                allowed_origin: req.allowed_origin,
            })
            .await?
            .into_inner()
            .handle;
        let mut stream = self
            .queryable_client()
            .events(pb::QueryableEventsRequest {
                client_id: self.inner.client_id.clone(),
                handle,
            })
            .await?
            .into_inner();
        let (tx, rx) = bounded_drop_oldest(RECV_QUEUE_CAPACITY);
        tokio::spawn(async move {
            while let Ok(Some(event)) = stream.message().await {
                let _ = tx.push(event);
            }
        });
        let (write_tx, write_rx) = bounded_drop_oldest(SEND_QUEUE_CAPACITY);
        let queryable = GrpcQueryable {
            session: self.clone(),
            handle,
            rx,
            write_tx,
        };
        queryable.spawn_worker(write_rx);
        Ok(queryable)
    }

    pub async fn declare_querier(&self, req: DeclareQuerierArgs) -> Result<GrpcQuerier, Error> {
        let handle = self
            .querier_client()
            .declare_querier(pb::DeclareQuerierRequest {
                client_id: self.inner.client_id.clone(),
                key_expr: req.key_expr,
                target: req.target,
                consolidation: req.consolidation,
                timeout_ms: req.timeout_ms,
                allowed_destination: req.allowed_destination,
            })
            .await?
            .into_inner()
            .handle;
        Ok(GrpcQuerier {
            session: self.clone(),
            handle,
        })
    }
}

impl Drop for GrpcSession {
    fn drop(&mut self) {
        if Arc::strong_count(&self.inner) == 1 {
            let channel = self.inner.channel.clone();
            let client_id = self.inner.client_id.clone();
            let cleanup = async move {
                let _ = SessionServiceClient::new(channel)
                    .cleanup_client(pb::CleanupClientRequest { client_id })
                    .await;
            };
            if let Ok(handle) = tokio::runtime::Handle::try_current() {
                handle.spawn(cleanup);
            } else {
                std::thread::spawn(move || {
                    if let Ok(rt) = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                    {
                        rt.block_on(cleanup);
                    }
                });
            }
        }
    }
}

impl GrpcPublisher {
    fn spawn_worker(&self, rx: DropOldestReceiver<PublisherCommand>) {
        let publisher = self.clone();
        tokio::spawn(async move {
            while let Ok(command) = rx.recv_async().await {
                match command {
                    PublisherCommand::Put(req) => {
                        let _ = publisher
                            .session
                            .publisher_client()
                            .put(pb::PublisherPutRequest {
                                client_id: publisher.session.inner.client_id.clone(),
                                handle: publisher.handle,
                                payload: req.payload,
                                encoding: req.encoding,
                                attachment: req.attachment,
                                timestamp: req.timestamp,
                            })
                            .await;
                        rx.processed_one();
                    }
                    PublisherCommand::Delete(req) => {
                        let _ = publisher
                            .session
                            .publisher_client()
                            .delete(pb::PublisherDeleteRequest {
                                client_id: publisher.session.inner.client_id.clone(),
                                handle: publisher.handle,
                                attachment: req.attachment,
                                timestamp: req.timestamp,
                            })
                            .await;
                        rx.processed_one();
                    }
                }
            }
        });
    }

    pub fn handle(&self) -> u64 {
        self.handle
    }

    pub fn send_dropped_count(&self) -> u64 {
        self.write_tx.dropped_count()
    }

    pub async fn put(&self, req: PublisherPutArgs) -> Result<(), Error> {
        self.write_tx
            .push(PublisherCommand::Put(req))
            .map_err(|_| tonic::Status::unavailable("publisher send queue closed").into())
    }

    pub async fn delete(&self, req: PublisherDeleteArgs) -> Result<(), Error> {
        self.write_tx
            .push(PublisherCommand::Delete(req))
            .map_err(|_| tonic::Status::unavailable("publisher send queue closed").into())
    }

    pub async fn undeclare(&self) -> Result<(), Error> {
        self.write_tx.wait_empty().await;
        self.session
            .publisher_client()
            .undeclare(pb::UndeclareRequest {
                client_id: self.session.inner.client_id.clone(),
                handle: self.handle,
            })
            .await?;
        Ok(())
    }
}

impl GrpcSubscriber {
    pub fn handle(&self) -> u64 {
        self.handle
    }

    pub fn receiver(&self) -> &DropOldestReceiver<pb::SubscriberEvent> {
        &self.rx
    }

    pub fn dropped_count(&self) -> u64 {
        self.rx.dropped_count()
    }

    pub async fn undeclare(&self) -> Result<(), Error> {
        self.session
            .subscriber_client()
            .undeclare(pb::UndeclareRequest {
                client_id: self.session.inner.client_id.clone(),
                handle: self.handle,
            })
            .await?;
        Ok(())
    }
}

impl GrpcQueryable {
    fn spawn_worker(&self, rx: DropOldestReceiver<QueryableCommand>) {
        let queryable = self.clone();
        tokio::spawn(async move {
            while let Ok(command) = rx.recv_async().await {
                match command {
                    QueryableCommand::Reply(req) => {
                        let _ = queryable
                            .session
                            .queryable_client()
                            .reply(pb::QueryReplyRequest {
                                client_id: queryable.session.inner.client_id.clone(),
                                handle: queryable.handle,
                                query_id: req.query_id,
                                key_expr: req.key_expr,
                                payload: req.payload,
                                encoding: req.encoding,
                                attachment: req.attachment,
                                timestamp: req.timestamp,
                            })
                            .await;
                        rx.processed_one();
                    }
                    QueryableCommand::ReplyErr(req) => {
                        let _ = queryable
                            .session
                            .queryable_client()
                            .reply_err(pb::QueryReplyErrRequest {
                                client_id: queryable.session.inner.client_id.clone(),
                                handle: queryable.handle,
                                query_id: req.query_id,
                                payload: req.payload,
                                encoding: req.encoding,
                            })
                            .await;
                        rx.processed_one();
                    }
                    QueryableCommand::ReplyDelete(req) => {
                        let _ = queryable
                            .session
                            .queryable_client()
                            .reply_delete(pb::QueryReplyDeleteRequest {
                                client_id: queryable.session.inner.client_id.clone(),
                                handle: queryable.handle,
                                query_id: req.query_id,
                                key_expr: req.key_expr,
                                attachment: req.attachment,
                                timestamp: req.timestamp,
                            })
                            .await;
                        rx.processed_one();
                    }
                }
            }
        });
    }

    pub fn handle(&self) -> u64 {
        self.handle
    }

    pub fn send_dropped_count(&self) -> u64 {
        self.write_tx.dropped_count()
    }

    pub fn receiver(&self) -> &DropOldestReceiver<pb::QueryableEvent> {
        &self.rx
    }

    pub fn dropped_count(&self) -> u64 {
        self.rx.dropped_count()
    }

    pub async fn reply(&self, req: QueryReplyArgs) -> Result<(), Error> {
        self.write_tx
            .push(QueryableCommand::Reply(req))
            .map_err(|_| tonic::Status::unavailable("queryable send queue closed").into())
    }

    pub async fn reply_err(&self, req: QueryReplyErrArgs) -> Result<(), Error> {
        self.write_tx
            .push(QueryableCommand::ReplyErr(req))
            .map_err(|_| tonic::Status::unavailable("queryable send queue closed").into())
    }

    pub async fn reply_delete(&self, req: QueryReplyDeleteArgs) -> Result<(), Error> {
        self.write_tx
            .push(QueryableCommand::ReplyDelete(req))
            .map_err(|_| tonic::Status::unavailable("queryable send queue closed").into())
    }

    pub async fn undeclare(&self) -> Result<(), Error> {
        self.write_tx.wait_empty().await;
        self.session
            .queryable_client()
            .undeclare(pb::UndeclareRequest {
                client_id: self.session.inner.client_id.clone(),
                handle: self.handle,
            })
            .await?;
        Ok(())
    }
}

impl GrpcQuerier {
    pub fn handle(&self) -> u64 {
        self.handle
    }

    pub async fn get(&self, req: QuerierGetArgs) -> Result<DropOldestReceiver<pb::Reply>, Error> {
        let mut stream = self
            .session
            .querier_client()
            .get(pb::QuerierGetRequest {
                client_id: self.session.inner.client_id.clone(),
                handle: self.handle,
                parameters: req.parameters,
                payload: req.payload,
                encoding: req.encoding,
                attachment: req.attachment,
            })
            .await?
            .into_inner();
        let (tx, rx) = bounded_drop_oldest(RECV_QUEUE_CAPACITY);
        tokio::spawn(async move {
            while let Ok(Some(reply)) = stream.message().await {
                let _ = tx.push(reply);
            }
        });
        Ok(rx)
    }

    pub async fn undeclare(&self) -> Result<(), Error> {
        self.session
            .querier_client()
            .undeclare(pb::UndeclareRequest {
                client_id: self.session.inner.client_id.clone(),
                handle: self.handle,
            })
            .await?;
        Ok(())
    }
}
