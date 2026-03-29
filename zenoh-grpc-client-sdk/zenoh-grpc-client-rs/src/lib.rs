mod args;
mod queue;

use std::{
    panic::{catch_unwind, AssertUnwindSafe},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
};

use hyper_util::rt::TokioIo;
use thiserror::Error;
use tokio::net::UnixStream;
use tonic::transport::{Channel, Endpoint};
use tower::service_fn;
use uuid::Uuid;
use zenoh_grpc_proto::v1::{
    self as pb, publisher_service_client::PublisherServiceClient,
    querier_service_client::QuerierServiceClient, queryable_service_client::QueryableServiceClient,
    session_service_client::SessionServiceClient,
    subscriber_service_client::SubscriberServiceClient,
};

pub use args::*;
pub use pb::{
    CongestionControl, ConsolidationMode, Locality, Priority, QueryTarget, Reliability, SampleKind,
};
pub use queue::DropOldestReceiver;
use queue::{bounded_drop_oldest, DropOldestSender};

pub type SubscriberCallback = Arc<dyn Fn(pb::SubscriberEvent) + Send + Sync + 'static>;
pub type QueryableCallback = Arc<dyn Fn(GrpcQuery) + Send + Sync + 'static>;

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
    #[error("{kind} is callback-driven")]
    CallbackDriven { kind: &'static str },
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
    mode: ConsumerMode,
}

#[derive(Clone)]
pub struct GrpcQueryable {
    session: GrpcSession,
    handle: u64,
    rx: DropOldestReceiver<pb::QueryableEvent>,
    write_tx: DropOldestSender<QueryableCommand>,
    mode: ConsumerMode,
}

#[derive(Clone)]
pub struct GrpcQuerier {
    session: GrpcSession,
    handle: u64,
}

#[derive(Clone)]
pub struct ReplyStream {
    rx: DropOldestReceiver<pb::Reply>,
}

#[derive(Clone)]
pub struct QueryStream {
    queryable: GrpcQueryable,
    rx: DropOldestReceiver<pb::QueryableEvent>,
}

pub struct GrpcQuery {
    queryable: GrpcQueryable,
    inner: pb::Query,
    finished: Arc<AtomicBool>,
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
    Finish { query_id: u64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConsumerMode {
    Pull,
    Callback,
}

const SEND_QUEUE_CAPACITY: usize = 256;
const RECV_QUEUE_CAPACITY: usize = 256;

fn spawn_subscriber_callback_dispatcher(
    rx: DropOldestReceiver<pb::SubscriberEvent>,
    callback: SubscriberCallback,
) {
    thread::spawn(move || {
        while let Ok(event) = rx.recv() {
            if catch_unwind(AssertUnwindSafe(|| (callback)(event))).is_err() {
                eprintln!("zenoh-grpc-client-rs: subscriber callback panicked");
            }
        }
    });
}

fn spawn_queryable_callback_dispatcher(
    queryable: GrpcQueryable,
    rx: DropOldestReceiver<pb::QueryableEvent>,
    callback: QueryableCallback,
) {
    thread::spawn(move || {
        while let Ok(event) = rx.recv() {
            let result = queryable.query_from_event(event);
            if catch_unwind(AssertUnwindSafe(|| {
                if let Ok(query) = result {
                    (callback)(query)
                }
            }))
            .is_err()
            {
                eprintln!("zenoh-grpc-client-rs: queryable callback panicked");
            }
        }
    });
}

async fn connect_channel(addr: &ConnectAddr) -> Result<Channel, tonic::transport::Error> {
    match addr {
        ConnectAddr::Tcp(addr) => {
            Endpoint::from_shared(format!("http://{addr}"))?
                .connect()
                .await
        }
        ConnectAddr::Unix(path) => {
            Endpoint::try_from("http://[::]:50051")?
                .connect_with_connector(service_fn({
                    let path = path.clone();
                    move |_| {
                        let path = path.clone();
                        async move { UnixStream::connect(path).await.map(TokioIo::new) }
                    }
                }))
                .await
        }
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
        let session = Self { inner, write_tx };
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

    pub async fn get(&self, req: SessionGetArgs) -> Result<ReplyStream, Error> {
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
        Ok(ReplyStream { rx })
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

    pub async fn declare_publisher(
        &self,
        req: DeclarePublisherArgs,
    ) -> Result<GrpcPublisher, Error> {
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

    pub async fn declare_subscriber(
        &self,
        req: DeclareSubscriberArgs,
        callback: Option<SubscriberCallback>,
    ) -> Result<GrpcSubscriber, Error> {
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
        let subscriber = GrpcSubscriber {
            session: self.clone(),
            handle,
            rx,
            mode: if callback.is_some() {
                ConsumerMode::Callback
            } else {
                ConsumerMode::Pull
            },
        };
        if let Some(callback) = callback {
            spawn_subscriber_callback_dispatcher(subscriber.rx.clone(), callback);
        }
        Ok(subscriber)
    }

    pub async fn declare_queryable(
        &self,
        req: DeclareQueryableArgs,
        callback: Option<QueryableCallback>,
    ) -> Result<GrpcQueryable, Error> {
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
            mode: if callback.is_some() {
                ConsumerMode::Callback
            } else {
                ConsumerMode::Pull
            },
        };
        if let Some(callback) = callback {
            spawn_queryable_callback_dispatcher(queryable.clone(), queryable.rx.clone(), callback);
        }
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
    fn ensure_pull_mode(&self) -> Result<(), Error> {
        if self.mode == ConsumerMode::Callback {
            Err(Error::CallbackDriven { kind: "subscriber" })
        } else {
            Ok(())
        }
    }

    pub fn handle(&self) -> u64 {
        self.handle
    }

    pub fn receiver(&self) -> Result<&DropOldestReceiver<pb::SubscriberEvent>, Error> {
        self.ensure_pull_mode()?;
        Ok(&self.rx)
    }

    pub fn recv(&self) -> Result<pb::SubscriberEvent, Error> {
        self.ensure_pull_mode()?;
        self.rx
            .recv()
            .map_err(|_| tonic::Status::unavailable("subscriber event queue closed").into())
    }

    pub fn try_recv(&self) -> Result<Option<pb::SubscriberEvent>, Error> {
        self.ensure_pull_mode()?;
        match self.rx.try_recv() {
            Ok(event) => Ok(Some(event)),
            Err(flume::TryRecvError::Empty) => Ok(None),
            Err(flume::TryRecvError::Disconnected) => {
                Err(tonic::Status::unavailable("subscriber event queue closed").into())
            }
        }
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
    fn ensure_pull_mode(&self) -> Result<(), Error> {
        if self.mode == ConsumerMode::Callback {
            Err(Error::CallbackDriven { kind: "queryable" })
        } else {
            Ok(())
        }
    }

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
                    QueryableCommand::Finish { query_id } => {
                        let _ = queryable
                            .session
                            .queryable_client()
                            .finish(pb::QueryFinishRequest {
                                client_id: queryable.session.inner.client_id.clone(),
                                handle: queryable.handle,
                                query_id,
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

    pub fn receiver(&self) -> Result<QueryStream, Error> {
        self.ensure_pull_mode()?;
        Ok(QueryStream {
            queryable: self.clone(),
            rx: self.rx.clone(),
        })
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

    fn query_from_event(&self, event: pb::QueryableEvent) -> Result<GrpcQuery, Error> {
        let inner = event
            .query
            .ok_or_else(|| tonic::Status::unavailable("queryable event missing query"))?;
        Ok(GrpcQuery {
            queryable: self.clone(),
            inner,
            finished: Arc::new(AtomicBool::new(false)),
        })
    }
}

impl GrpcQuerier {
    pub fn handle(&self) -> u64 {
        self.handle
    }

    pub async fn get(&self, req: QuerierGetArgs) -> Result<ReplyStream, Error> {
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
        Ok(ReplyStream { rx })
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

impl ReplyStream {
    pub fn recv(&self) -> Result<pb::Reply, Error> {
        self.rx
            .recv()
            .map_err(|_| tonic::Status::unavailable("reply stream closed").into())
    }

    pub fn try_recv(&self) -> Result<Option<pb::Reply>, Error> {
        match self.rx.try_recv() {
            Ok(reply) => Ok(Some(reply)),
            Err(flume::TryRecvError::Empty) => Ok(None),
            Err(flume::TryRecvError::Disconnected) => {
                Err(tonic::Status::unavailable("reply stream closed").into())
            }
        }
    }

    pub async fn recv_async(&self) -> Result<pb::Reply, Error> {
        self.rx
            .recv_async()
            .await
            .map_err(|_| tonic::Status::unavailable("reply stream closed").into())
    }

    pub fn dropped_count(&self) -> u64 {
        self.rx.dropped_count()
    }

    pub fn is_closed(&self) -> bool {
        self.rx.is_disconnected()
    }
}

impl QueryStream {
    pub fn recv(&self) -> Result<GrpcQuery, Error> {
        let event = self
            .rx
            .recv()
            .map_err(|_| tonic::Status::unavailable("query stream closed"))?;
        self.queryable.query_from_event(event)
    }

    pub fn try_recv(&self) -> Result<Option<GrpcQuery>, Error> {
        match self.rx.try_recv() {
            Ok(event) => Ok(Some(self.queryable.query_from_event(event)?)),
            Err(flume::TryRecvError::Empty) => Ok(None),
            Err(flume::TryRecvError::Disconnected) => {
                Err(tonic::Status::unavailable("query stream closed").into())
            }
        }
    }

    pub async fn recv_async(&self) -> Result<GrpcQuery, Error> {
        let event = self
            .rx
            .recv_async()
            .await
            .map_err(|_| tonic::Status::unavailable("query stream closed"))?;
        self.queryable.query_from_event(event)
    }

    pub fn dropped_count(&self) -> u64 {
        self.rx.dropped_count()
    }

    pub fn is_closed(&self) -> bool {
        self.rx.is_disconnected()
    }
}

impl GrpcQuery {
    fn enqueue_command(&self, command: QueryableCommand) -> Result<(), Error> {
        self.queryable
            .write_tx
            .push(command)
            .map_err(|_| tonic::Status::unavailable("queryable send queue closed").into())
    }

    fn mark_finished(&self) -> bool {
        !self.finished.swap(true, Ordering::AcqRel)
    }

    pub fn query_id(&self) -> u64 {
        self.inner.query_id
    }

    pub fn selector(&self) -> &str {
        &self.inner.selector
    }

    pub fn key_expr(&self) -> &str {
        &self.inner.key_expr
    }

    pub fn parameters(&self) -> &str {
        &self.inner.parameters
    }

    pub fn payload(&self) -> &[u8] {
        &self.inner.payload
    }

    pub fn encoding(&self) -> &str {
        &self.inner.encoding
    }

    pub fn attachment(&self) -> &[u8] {
        &self.inner.attachment
    }

    pub async fn reply(
        &self,
        key_expr: impl Into<String>,
        payload: impl Into<Vec<u8>>,
        encoding: impl Into<String>,
        attachment: impl Into<Vec<u8>>,
        timestamp: impl Into<String>,
    ) -> Result<(), Error> {
        self.enqueue_command(QueryableCommand::Reply(QueryReplyArgs {
            query_id: self.inner.query_id,
            key_expr: key_expr.into(),
            payload: payload.into(),
            encoding: encoding.into(),
            attachment: attachment.into(),
            timestamp: timestamp.into(),
        }))
    }

    pub async fn reply_err(
        &self,
        payload: impl Into<Vec<u8>>,
        encoding: impl Into<String>,
    ) -> Result<(), Error> {
        self.enqueue_command(QueryableCommand::ReplyErr(QueryReplyErrArgs {
            query_id: self.inner.query_id,
            payload: payload.into(),
            encoding: encoding.into(),
        }))
    }

    pub async fn reply_delete(
        &self,
        key_expr: impl Into<String>,
        attachment: impl Into<Vec<u8>>,
        timestamp: impl Into<String>,
    ) -> Result<(), Error> {
        self.enqueue_command(QueryableCommand::ReplyDelete(QueryReplyDeleteArgs {
            query_id: self.inner.query_id,
            key_expr: key_expr.into(),
            attachment: attachment.into(),
            timestamp: timestamp.into(),
        }))
    }

    pub async fn finish(self) -> Result<(), Error> {
        if self.mark_finished() {
            self.enqueue_command(QueryableCommand::Finish {
                query_id: self.inner.query_id,
            })?;
        }
        Ok(())
    }
}

impl Drop for GrpcQuery {
    fn drop(&mut self) {
        if self.mark_finished() {
            let _ = self.enqueue_command(QueryableCommand::Finish {
                query_id: self.inner.query_id,
            });
        }
    }
}
