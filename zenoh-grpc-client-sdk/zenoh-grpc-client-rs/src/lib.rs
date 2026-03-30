mod args;
mod queue;

use std::{
    collections::HashMap,
    collections::HashSet,
    panic::{catch_unwind, AssertUnwindSafe},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};

use hyper_util::rt::TokioIo;
use thiserror::Error;
use tokio::{
    net::UnixStream,
    sync::{Mutex, Notify},
};
use tonic::{transport::{Channel, Endpoint}, Code, Status};
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
pub struct GrpcSession {
    inner: Arc<Inner>,
    write_tx: DropOldestSender<SessionCommand>,
}

#[derive(Clone)]
pub struct GrpcPublisher {
    inner: Arc<PublisherInner>,
    write_tx: DropOldestSender<PublisherCommand>,
}

#[derive(Clone)]
pub struct GrpcSubscriber {
    inner: Arc<SubscriberInner>,
    rx: DropOldestReceiver<pb::SubscriberEvent>,
    mode: ConsumerMode,
}

#[derive(Clone)]
pub struct GrpcQueryable {
    inner: Arc<QueryableInner>,
    rx: DropOldestReceiver<QueryableEventEnvelope>,
    write_tx: DropOldestSender<QueryableCommand>,
    mode: ConsumerMode,
}

#[derive(Clone)]
pub struct GrpcQuerier {
    inner: Arc<QuerierInner>,
}

#[derive(Clone)]
pub struct ReplyStream {
    rx: DropOldestReceiver<pb::Reply>,
}

pub struct GrpcQuery {
    queryable: GrpcQueryable,
    inner: pb::Query,
    remote_handle: u64,
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
    Reply {
        identity: QueryIdentity,
        args: QueryReplyArgs,
    },
    ReplyErr {
        identity: QueryIdentity,
        args: QueryReplyErrArgs,
    },
    ReplyDelete {
        identity: QueryIdentity,
        args: QueryReplyDeleteArgs,
    },
    Finish {
        identity: QueryIdentity,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConsumerMode {
    Pull,
    Callback,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct QueryIdentity {
    remote_handle: u64,
    query_id: u64,
}

#[derive(Debug, Clone)]
struct RemoteBinding {
    handle: Option<u64>,
    generation: u64,
}

#[derive(Debug, Clone)]
struct ConnectionState {
    channel: Option<Channel>,
    generation: u64,
}

struct Inner {
    addr: ConnectAddr,
    client_id: String,
    next_local_handle: AtomicU64,
    closed: AtomicBool,
    warning_issued: AtomicBool,
    connection: Mutex<ConnectionState>,
    connection_notify: Notify,
}

struct PublisherInner {
    session: GrpcSession,
    local_handle: u64,
    args: DeclarePublisherArgs,
    binding: Mutex<RemoteBinding>,
    closed: AtomicBool,
    close_notify: Notify,
}

struct SubscriberInner {
    session: GrpcSession,
    local_handle: u64,
    args: DeclareSubscriberArgs,
    binding: Mutex<RemoteBinding>,
    event_tx: Mutex<Option<DropOldestSender<pb::SubscriberEvent>>>,
    stream_ready: AtomicBool,
    ready_notify: Notify,
    closed: AtomicBool,
    close_notify: Notify,
}

struct QueryableInner {
    session: GrpcSession,
    local_handle: u64,
    args: DeclareQueryableArgs,
    binding: Mutex<RemoteBinding>,
    event_tx: Mutex<Option<DropOldestSender<QueryableEventEnvelope>>>,
    stale_queries: Mutex<HashSet<QueryIdentity>>,
    active_queries: std::sync::Mutex<HashMap<u64, QueryIdentity>>,
    stream_ready: AtomicBool,
    ready_notify: Notify,
    closed: AtomicBool,
    close_notify: Notify,
}

struct QuerierInner {
    session: GrpcSession,
    local_handle: u64,
    args: DeclareQuerierArgs,
    binding: Mutex<RemoteBinding>,
    closed: AtomicBool,
    close_notify: Notify,
}

#[derive(Debug, Clone)]
struct QueryableEventEnvelope {
    remote_handle: u64,
    event: pb::QueryableEvent,
}

const SEND_QUEUE_CAPACITY: usize = 256;
const RECV_QUEUE_CAPACITY: usize = 256;
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(2);// Should be less than client_lease_duration to ensure timely cleanup of dead clients
const RECONNECT_DELAY_INITIAL: Duration = Duration::from_millis(200);
const RECONNECT_DELAY_MAX: Duration = Duration::from_secs(2);
const INITIAL_CONNECT_TIMEOUT: Duration = Duration::from_millis(200);
const STREAM_READY_TIMEOUT: Duration = Duration::from_millis(500);
const DISCONNECTED_WARNING: &str =
    "zenoh-grpc-client-rs: disconnected from zenoh gRPC server, reconnecting in background";

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
    rx: DropOldestReceiver<QueryableEventEnvelope>,
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

fn validate_connect_addr(addr: &ConnectAddr) -> Result<(), tonic::transport::Error> {
    match addr {
        ConnectAddr::Tcp(addr) => {
            let _ = Endpoint::from_shared(format!("http://{addr}"))?;
        }
        ConnectAddr::Unix(_) => {
            let _ = Endpoint::try_from("http://[::]:50051")?;
        }
    }
    Ok(())
}

fn is_disconnect_status(status: &Status) -> bool {
    matches!(
        status.code(),
        Code::Unavailable | Code::Unknown | Code::Cancelled
    )
}

fn is_stale_status(status: &Status) -> bool {
    matches!(status.code(), Code::NotFound | Code::PermissionDenied)
}

fn warn_yellow(message: &str) {
    eprintln!("\x1b[33m{message}\x1b[0m");
}

fn info_green(message: &str) {
    eprintln!("\x1b[32m{message}\x1b[0m");
}

fn log_operation_warning(context: &str, status: &Status) {
    eprintln!(
        "zenoh-grpc-client-rs: {context} failed: {} ({})",
        status.message(),
        status.code()
    );
}

fn empty_reply_stream() -> ReplyStream {
    let (_tx, rx) = bounded_drop_oldest(RECV_QUEUE_CAPACITY);
    ReplyStream { rx }
}

fn session_client(channel: Channel) -> SessionServiceClient<Channel> {
    SessionServiceClient::new(channel)
}

fn publisher_client(channel: Channel) -> PublisherServiceClient<Channel> {
    PublisherServiceClient::new(channel)
}

fn subscriber_client(channel: Channel) -> SubscriberServiceClient<Channel> {
    SubscriberServiceClient::new(channel)
}

fn queryable_client(channel: Channel) -> QueryableServiceClient<Channel> {
    QueryableServiceClient::new(channel)
}

fn querier_client(channel: Channel) -> QuerierServiceClient<Channel> {
    QuerierServiceClient::new(channel)
}

impl GrpcSession {
    pub async fn connect(addr: ConnectAddr) -> Result<Self, Error> {
        validate_connect_addr(&addr)?;
        let inner = Arc::new(Inner {
            addr,
            client_id: Uuid::new_v4().to_string(),
            next_local_handle: AtomicU64::new(1),
            closed: AtomicBool::new(false),
            warning_issued: AtomicBool::new(false),
            connection: Mutex::new(ConnectionState {
                channel: None,
                generation: 0,
            }),
            connection_notify: Notify::new(),
        });
        let (write_tx, write_rx) = bounded_drop_oldest(SEND_QUEUE_CAPACITY);
        let session = Self { inner, write_tx };
        if let Ok(Ok(channel)) =
            tokio::time::timeout(INITIAL_CONNECT_TIMEOUT, connect_channel(&session.inner.addr)).await
        {
            session.set_connected(channel).await;
        }
        session.spawn_reconnect_worker();
        session.spawn_heartbeat_worker();
        session.spawn_session_worker(write_rx);
        Ok(session)
    }

    fn client_id_owned(&self) -> String {
        self.inner.client_id.clone()
    }

    fn is_closed(&self) -> bool {
        self.inner.closed.load(Ordering::Acquire)
    }

    fn alloc_local_handle(&self) -> u64 {
        self.inner.next_local_handle.fetch_add(1, Ordering::Relaxed)
    }

    fn warn_disconnected_once(&self) {
        if !self.inner.warning_issued.swap(true, Ordering::AcqRel) {
            warn_yellow(DISCONNECTED_WARNING);
        }
    }

    async fn current_connection(&self) -> Option<(Channel, u64)> {
        let state = self.inner.connection.lock().await;
        state.channel.clone().map(|channel| (channel, state.generation))
    }

    async fn wait_for_connection_or_closed(&self) -> Option<(Channel, u64)> {
        loop {
            if self.is_closed() {
                return None;
            }
            if let Some(connection) = self.current_connection().await {
                return Some(connection);
            }
            self.inner.connection_notify.notified().await;
        }
    }

    async fn set_connected(&self, channel: Channel) {
        let was_disconnected = self.inner.warning_issued.swap(false, Ordering::AcqRel);
        let mut state = self.inner.connection.lock().await;
        state.generation += 1;
        state.channel = Some(channel);
        drop(state);
        if was_disconnected {
            info_green("zenoh-grpc-client-rs: reconnected to zenoh gRPC server");
        }
        self.inner.connection_notify.notify_waiters();
    }

    async fn mark_disconnected(&self, generation: u64) {
        let mut state = self.inner.connection.lock().await;
        if state.generation == generation && state.channel.is_some() {
            state.channel = None;
            drop(state);
            self.warn_disconnected_once();
            self.inner.connection_notify.notify_waiters();
        }
    }

    fn spawn_reconnect_worker(&self) {
        let session = self.clone();
        tokio::spawn(async move {
            let mut delay = RECONNECT_DELAY_INITIAL;
            loop {
                if session.is_closed() {
                    break;
                }

                if session.current_connection().await.is_some() {
                    session.inner.connection_notify.notified().await;
                    continue;
                }

                match connect_channel(&session.inner.addr).await {
                    Ok(channel) => {
                        session.set_connected(channel).await;
                        delay = RECONNECT_DELAY_INITIAL;
                    }
                    Err(_) => {
                        session.warn_disconnected_once();
                        tokio::select! {
                            _ = tokio::time::sleep(delay) => {}
                            _ = session.inner.connection_notify.notified() => {}
                        }
                        delay = (delay + delay).min(RECONNECT_DELAY_MAX);
                    }
                }
            }
        });
    }

    fn spawn_heartbeat_worker(&self) {
        let session = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(HEARTBEAT_INTERVAL);
            loop {
                interval.tick().await;
                if session.is_closed() {
                    break;
                }

                let Some((channel, generation)) = session.current_connection().await else {
                    continue;
                };
                let result = session_client(channel)
                    .touch_client(pb::TouchClientRequest {
                        client_id: session.client_id_owned(),
                    })
                    .await;
                if let Err(status) = result {
                    if is_disconnect_status(&status) {
                        session.mark_disconnected(generation).await;
                    }
                }
            }
        });
    }

    fn spawn_session_worker(&self, rx: DropOldestReceiver<SessionCommand>) {
        let session = self.clone();
        tokio::spawn(async move {
            while let Ok(command) = rx.recv_async().await {
                loop {
                    if session.is_closed() {
                        rx.processed_one();
                        break;
                    }
                    let Some((channel, generation)) = session.wait_for_connection_or_closed().await
                    else {
                        rx.processed_one();
                        break;
                    };
                    let result = match &command {
                        SessionCommand::Put(req) => session_client(channel.clone())
                            .put(pb::SessionPutRequest {
                                client_id: session.client_id_owned(),
                                key_expr: req.key_expr.clone(),
                                payload: req.payload.clone(),
                                encoding: req.encoding.clone(),
                                congestion_control: req.congestion_control,
                                priority: req.priority,
                                express: req.express,
                                attachment: req.attachment.clone(),
                                timestamp: req.timestamp.clone(),
                                allowed_destination: req.allowed_destination,
                            })
                            .await
                            .map(|_| ()),
                        SessionCommand::Delete(req) => session_client(channel.clone())
                            .delete(pb::SessionDeleteRequest {
                                client_id: session.client_id_owned(),
                                key_expr: req.key_expr.clone(),
                                congestion_control: req.congestion_control,
                                priority: req.priority,
                                express: req.express,
                                attachment: req.attachment.clone(),
                                timestamp: req.timestamp.clone(),
                                allowed_destination: req.allowed_destination,
                            })
                            .await
                            .map(|_| ()),
                    };

                    match result {
                        Ok(()) => {
                            rx.processed_one();
                            break;
                        }
                        Err(status) if is_disconnect_status(&status) => {
                            session.mark_disconnected(generation).await;
                        }
                        Err(status) => {
                            log_operation_warning("session request", &status);
                            rx.processed_one();
                            break;
                        }
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
        loop {
            let Some((channel, generation)) = self.wait_for_connection_or_closed().await else {
                return Err(Status::unavailable("session closed").into());
            };
            match session_client(channel)
                .info(pb::SessionInfoRequest {
                    client_id: self.client_id_owned(),
                })
                .await
            {
                Ok(reply) => return Ok(reply.into_inner()),
                Err(status) if is_disconnect_status(&status) => {
                    self.mark_disconnected(generation).await;
                }
                Err(status) => return Err(status.into()),
            }
        }
    }

    pub async fn put(&self, req: SessionPutArgs) -> Result<(), Error> {
        self.write_tx
            .push(SessionCommand::Put(req))
            .map_err(|_| Status::unavailable("session send queue closed").into())
    }

    pub async fn delete(&self, req: SessionDeleteArgs) -> Result<(), Error> {
        self.write_tx
            .push(SessionCommand::Delete(req))
            .map_err(|_| Status::unavailable("session send queue closed").into())
    }

    pub async fn get(&self, req: SessionGetArgs) -> Result<ReplyStream, Error> {
        let Some((channel, generation)) = self.current_connection().await else {
            self.warn_disconnected_once();
            return Ok(empty_reply_stream());
        };
        let response = session_client(channel)
            .get(pb::SessionGetRequest {
                client_id: self.client_id_owned(),
                selector: req.selector,
                target: req.target,
                consolidation: req.consolidation,
                timeout_ms: req.timeout_ms,
                payload: req.payload,
                encoding: req.encoding,
                attachment: req.attachment,
                allowed_destination: req.allowed_destination,
            })
            .await;

        let mut stream = match response {
            Ok(response) => response.into_inner(),
            Err(status) if is_disconnect_status(&status) => {
                self.mark_disconnected(generation).await;
                return Ok(empty_reply_stream());
            }
            Err(status) => return Err(status.into()),
        };

        let (tx, rx) = bounded_drop_oldest(RECV_QUEUE_CAPACITY);
        let session = self.clone();
        tokio::spawn(async move {
            loop {
                match stream.message().await {
                    Ok(Some(reply)) => {
                        let _ = tx.push(reply);
                    }
                    Ok(None) => break,
                    Err(status) => {
                        if is_disconnect_status(&status) {
                            session.mark_disconnected(generation).await;
                        }
                        break;
                    }
                }
            }
        });
        Ok(ReplyStream { rx })
    }

    pub async fn cleanup(&self) -> Result<(), Error> {
        self.inner.closed.store(true, Ordering::Release);
        self.inner.connection_notify.notify_waiters();
        self.write_tx.wait_empty().await;

        let current = self.current_connection().await;
        if let Some((channel, generation)) = current {
            if let Err(status) = session_client(channel)
                .cleanup_client(pb::CleanupClientRequest {
                    client_id: self.client_id_owned(),
                })
                .await
            {
                if is_disconnect_status(&status) {
                    self.mark_disconnected(generation).await;
                    return Ok(());
                }
                return Err(status.into());
            }
        }
        Ok(())
    }

    pub async fn declare_publisher(
        &self,
        req: DeclarePublisherArgs,
    ) -> Result<GrpcPublisher, Error> {
        let inner = Arc::new(PublisherInner {
            session: self.clone(),
            local_handle: self.alloc_local_handle(),
            args: req,
            binding: Mutex::new(RemoteBinding {
                handle: None,
                generation: 0,
            }),
            closed: AtomicBool::new(false),
            close_notify: Notify::new(),
        });
        let (write_tx, write_rx) = bounded_drop_oldest(SEND_QUEUE_CAPACITY);
        let publisher = GrpcPublisher { inner, write_tx };
        publisher.spawn_worker(write_rx);
        Ok(publisher)
    }

    pub async fn declare_subscriber(
        &self,
        req: DeclareSubscriberArgs,
        callback: Option<SubscriberCallback>,
    ) -> Result<GrpcSubscriber, Error> {
        let (event_tx, rx) = bounded_drop_oldest(RECV_QUEUE_CAPACITY);
        let inner = Arc::new(SubscriberInner {
            session: self.clone(),
            local_handle: self.alloc_local_handle(),
            args: req,
            binding: Mutex::new(RemoteBinding {
                handle: None,
                generation: 0,
            }),
            event_tx: Mutex::new(Some(event_tx)),
            stream_ready: AtomicBool::new(false),
            ready_notify: Notify::new(),
            closed: AtomicBool::new(false),
            close_notify: Notify::new(),
        });
        let subscriber = GrpcSubscriber {
            inner,
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
        subscriber.spawn_event_worker();
        subscriber.wait_initial_stream_ready().await;
        Ok(subscriber)
    }

    pub async fn declare_queryable(
        &self,
        req: DeclareQueryableArgs,
        callback: Option<QueryableCallback>,
    ) -> Result<GrpcQueryable, Error> {
        let (event_tx, rx) = bounded_drop_oldest(RECV_QUEUE_CAPACITY);
        let (write_tx, write_rx) = bounded_drop_oldest(SEND_QUEUE_CAPACITY);
        let inner = Arc::new(QueryableInner {
            session: self.clone(),
            local_handle: self.alloc_local_handle(),
            args: req,
            binding: Mutex::new(RemoteBinding {
                handle: None,
                generation: 0,
            }),
            event_tx: Mutex::new(Some(event_tx)),
            stale_queries: Mutex::new(HashSet::new()),
            active_queries: std::sync::Mutex::new(HashMap::new()),
            stream_ready: AtomicBool::new(false),
            ready_notify: Notify::new(),
            closed: AtomicBool::new(false),
            close_notify: Notify::new(),
        });
        let queryable = GrpcQueryable {
            inner,
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
        queryable.spawn_event_worker();
        queryable.spawn_worker(write_rx);
        queryable.wait_initial_stream_ready().await;
        Ok(queryable)
    }

    pub async fn declare_querier(&self, req: DeclareQuerierArgs) -> Result<GrpcQuerier, Error> {
        Ok(GrpcQuerier {
            inner: Arc::new(QuerierInner {
                session: self.clone(),
                local_handle: self.alloc_local_handle(),
                args: req,
                binding: Mutex::new(RemoteBinding {
                    handle: None,
                    generation: 0,
                }),
                closed: AtomicBool::new(false),
                close_notify: Notify::new(),
            }),
        })
    }
}

impl Drop for GrpcSession {
    fn drop(&mut self) {
        if Arc::strong_count(&self.inner) != 1 {
            return;
        }

        self.inner.closed.store(true, Ordering::Release);
        self.inner.connection_notify.notify_waiters();

        let current = futures::executor::block_on(self.current_connection());
        let Some((channel, _generation)) = current else {
            return;
        };
        let client_id = self.client_id_owned();
        let cleanup = async move {
            let _ = session_client(channel)
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

impl PublisherInner {
    fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire) || self.session.is_closed()
    }

    async fn wait_for_connection_or_closed(&self) -> Option<(Channel, u64)> {
        loop {
            if self.is_closed() {
                return None;
            }
            if let Some(connection) = self.session.current_connection().await {
                return Some(connection);
            }
            tokio::select! {
                _ = self.session.inner.connection_notify.notified() => {}
                _ = self.close_notify.notified() => return None,
            }
        }
    }

    async fn ensure_remote_handle(&self, channel: Channel, generation: u64) -> Result<u64, Status> {
        let mut binding = self.binding.lock().await;
        if let Some(handle) = binding.handle {
            return Ok(handle);
        }
        let handle = publisher_client(channel)
            .declare_publisher(pb::DeclarePublisherRequest {
                client_id: self.session.client_id_owned(),
                key_expr: self.args.key_expr.clone(),
                encoding: self.args.encoding.clone(),
                congestion_control: self.args.congestion_control,
                priority: self.args.priority,
                express: self.args.express,
                reliability: self.args.reliability,
                allowed_destination: self.args.allowed_destination,
            })
            .await?
            .into_inner()
            .handle;
        binding.handle = Some(handle);
        binding.generation = generation;
        Ok(handle)
    }

    async fn invalidate_handle(&self, handle: u64) {
        let mut binding = self.binding.lock().await;
        if binding.handle == Some(handle) {
            binding.handle = None;
        }
    }

    async fn take_handle(&self) -> Option<u64> {
        self.binding.lock().await.handle.take()
    }
}

impl GrpcPublisher {
    fn spawn_worker(&self, rx: DropOldestReceiver<PublisherCommand>) {
        let publisher = self.clone();
        tokio::spawn(async move {
            while let Ok(command) = rx.recv_async().await {
                loop {
                    if publisher.inner.is_closed() {
                        rx.processed_one();
                        break;
                    }
                    let Some((channel, generation)) =
                        publisher.inner.wait_for_connection_or_closed().await
                    else {
                        rx.processed_one();
                        break;
                    };

                    let handle = match publisher
                        .inner
                        .ensure_remote_handle(channel.clone(), generation)
                        .await
                    {
                        Ok(handle) => handle,
                        Err(status) if is_disconnect_status(&status) => {
                            publisher.inner.session.mark_disconnected(generation).await;
                            continue;
                        }
                        Err(status) => {
                            log_operation_warning("publisher declare", &status);
                            rx.processed_one();
                            break;
                        }
                    };

                    let result = match &command {
                        PublisherCommand::Put(req) => publisher_client(channel.clone())
                            .put(pb::PublisherPutRequest {
                                client_id: publisher.inner.session.client_id_owned(),
                                handle,
                                payload: req.payload.clone(),
                                encoding: req.encoding.clone(),
                                attachment: req.attachment.clone(),
                                timestamp: req.timestamp.clone(),
                            })
                            .await
                            .map(|_| ()),
                        PublisherCommand::Delete(req) => publisher_client(channel.clone())
                            .delete(pb::PublisherDeleteRequest {
                                client_id: publisher.inner.session.client_id_owned(),
                                handle,
                                attachment: req.attachment.clone(),
                                timestamp: req.timestamp.clone(),
                            })
                            .await
                            .map(|_| ()),
                    };

                    match result {
                        Ok(()) => {
                            rx.processed_one();
                            break;
                        }
                        Err(status) if is_disconnect_status(&status) => {
                            publisher.inner.session.mark_disconnected(generation).await;
                        }
                        Err(status) if is_stale_status(&status) => {
                            publisher.inner.invalidate_handle(handle).await;
                        }
                        Err(status) => {
                            log_operation_warning("publisher request", &status);
                            rx.processed_one();
                            break;
                        }
                    }
                }
            }
        });
    }

    pub fn handle(&self) -> u64 {
        self.inner.local_handle
    }

    pub fn send_dropped_count(&self) -> u64 {
        self.write_tx.dropped_count()
    }

    pub async fn put(&self, req: PublisherPutArgs) -> Result<(), Error> {
        self.write_tx
            .push(PublisherCommand::Put(req))
            .map_err(|_| Status::unavailable("publisher send queue closed").into())
    }

    pub async fn delete(&self, req: PublisherDeleteArgs) -> Result<(), Error> {
        self.write_tx
            .push(PublisherCommand::Delete(req))
            .map_err(|_| Status::unavailable("publisher send queue closed").into())
    }

    pub async fn undeclare(&self) -> Result<(), Error> {
        self.write_tx.wait_empty().await;
        self.inner.closed.store(true, Ordering::Release);
        self.inner.close_notify.notify_waiters();

        let handle = self.inner.take_handle().await;
        let current = self.inner.session.current_connection().await;
        if let (Some(handle), Some((channel, generation))) = (handle, current) {
            if let Err(status) = publisher_client(channel)
                .undeclare(pb::UndeclareRequest {
                    client_id: self.inner.session.client_id_owned(),
                    handle,
                })
                .await
            {
                if is_disconnect_status(&status) {
                    self.inner.session.mark_disconnected(generation).await;
                } else if !is_stale_status(&status) {
                    return Err(status.into());
                }
            }
        }
        Ok(())
    }
}

impl SubscriberInner {
    fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire) || self.session.is_closed()
    }

    async fn wait_for_connection_or_closed(&self) -> Option<(Channel, u64)> {
        loop {
            if self.is_closed() {
                return None;
            }
            if let Some(connection) = self.session.current_connection().await {
                return Some(connection);
            }
            tokio::select! {
                _ = self.session.inner.connection_notify.notified() => {}
                _ = self.close_notify.notified() => return None,
            }
        }
    }

    async fn ensure_remote_handle(&self, channel: Channel, generation: u64) -> Result<u64, Status> {
        let mut binding = self.binding.lock().await;
        if let Some(handle) = binding.handle {
            return Ok(handle);
        }
        let handle = subscriber_client(channel)
            .declare_subscriber(pb::DeclareSubscriberRequest {
                client_id: self.session.client_id_owned(),
                key_expr: self.args.key_expr.clone(),
                allowed_origin: self.args.allowed_origin,
            })
            .await?
            .into_inner()
            .handle;
        binding.handle = Some(handle);
        binding.generation = generation;
        Ok(handle)
    }

    async fn invalidate_handle(&self, handle: u64) {
        let mut binding = self.binding.lock().await;
        if binding.handle == Some(handle) {
            binding.handle = None;
        }
    }

    async fn take_handle(&self) -> Option<u64> {
        self.binding.lock().await.handle.take()
    }

    async fn event_sender(&self) -> Option<DropOldestSender<pb::SubscriberEvent>> {
        self.event_tx.lock().await.as_ref().cloned()
    }

    async fn close_queue(&self) {
        self.event_tx.lock().await.take();
        self.stream_ready.store(false, Ordering::Release);
    }

    fn mark_stream_ready(&self) {
        self.stream_ready.store(true, Ordering::Release);
        self.ready_notify.notify_waiters();
    }

    fn mark_stream_not_ready(&self) {
        self.stream_ready.store(false, Ordering::Release);
    }

    async fn wait_initial_stream_ready(&self) {
        if self.session.current_connection().await.is_none() {
            return;
        }
        if self.stream_ready.load(Ordering::Acquire) {
            return;
        }
        let _ = tokio::time::timeout(STREAM_READY_TIMEOUT, async {
            while !self.stream_ready.load(Ordering::Acquire) && !self.is_closed() {
                self.ready_notify.notified().await;
            }
        })
        .await;
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

    fn spawn_event_worker(&self) {
        let subscriber = self.clone();
        tokio::spawn(async move {
            loop {
                if subscriber.inner.is_closed() {
                    break;
                }
                let Some((channel, generation)) = subscriber.inner.wait_for_connection_or_closed().await
                else {
                    break;
                };

                let handle = match subscriber
                    .inner
                    .ensure_remote_handle(channel.clone(), generation)
                    .await
                {
                    Ok(handle) => handle,
                    Err(status) if is_disconnect_status(&status) => {
                        subscriber.inner.session.mark_disconnected(generation).await;
                        continue;
                    }
                    Err(status) => {
                        log_operation_warning("subscriber declare", &status);
                        tokio::time::sleep(RECONNECT_DELAY_INITIAL).await;
                        continue;
                    }
                };

                let response = subscriber_client(channel.clone())
                    .events(pb::SubscriberEventsRequest {
                        client_id: subscriber.inner.session.client_id_owned(),
                        handle,
                    })
                    .await;
                let mut stream = match response {
                    Ok(response) => response.into_inner(),
                    Err(status) if is_disconnect_status(&status) => {
                        subscriber.inner.session.mark_disconnected(generation).await;
                        continue;
                    }
                    Err(status) if is_stale_status(&status) => {
                        subscriber.inner.invalidate_handle(handle).await;
                        continue;
                    }
                    Err(status) => {
                        log_operation_warning("subscriber events", &status);
                        tokio::time::sleep(RECONNECT_DELAY_INITIAL).await;
                        continue;
                    }
                };
                subscriber.inner.mark_stream_ready();

                loop {
                    let next = tokio::select! {
                        biased;
                        _ = subscriber.inner.close_notify.notified() => None,
                        next = stream.message() => Some(next),
                    };
                    let Some(next) = next else {
                        break;
                    };
                    match next {
                        Ok(Some(event)) => {
                            if let Some(tx) = subscriber.inner.event_sender().await {
                                let _ = tx.push(event);
                            } else {
                                break;
                            }
                        }
                        Ok(None) => {
                            subscriber.inner.mark_stream_not_ready();
                            subscriber.inner.session.mark_disconnected(generation).await;
                            break;
                        }
                        Err(status) if is_disconnect_status(&status) => {
                            subscriber.inner.mark_stream_not_ready();
                            subscriber.inner.session.mark_disconnected(generation).await;
                            break;
                        }
                        Err(status) if is_stale_status(&status) => {
                            subscriber.inner.mark_stream_not_ready();
                            subscriber.inner.invalidate_handle(handle).await;
                            break;
                        }
                        Err(status) => {
                            subscriber.inner.mark_stream_not_ready();
                            log_operation_warning("subscriber stream", &status);
                            break;
                        }
                    }
                }
            }
        });
    }

    async fn wait_initial_stream_ready(&self) {
        self.inner.wait_initial_stream_ready().await;
    }

    pub fn handle(&self) -> u64 {
        self.inner.local_handle
    }

    pub fn receiver(&self) -> Result<&DropOldestReceiver<pb::SubscriberEvent>, Error> {
        self.ensure_pull_mode()?;
        Ok(&self.rx)
    }

    pub fn recv(&self) -> Result<pb::SubscriberEvent, Error> {
        self.ensure_pull_mode()?;
        self.rx
            .recv()
            .map_err(|_| Status::unavailable("subscriber event queue closed").into())
    }

    pub fn try_recv(&self) -> Result<Option<pb::SubscriberEvent>, Error> {
        self.ensure_pull_mode()?;
        match self.rx.try_recv() {
            Ok(event) => Ok(Some(event)),
            Err(flume::TryRecvError::Empty) => Ok(None),
            Err(flume::TryRecvError::Disconnected) => {
                Err(Status::unavailable("subscriber event queue closed").into())
            }
        }
    }

    pub fn dropped_count(&self) -> u64 {
        self.rx.dropped_count()
    }

    pub async fn undeclare(&self) -> Result<(), Error> {
        self.inner.closed.store(true, Ordering::Release);
        self.inner.close_notify.notify_waiters();
        self.inner.close_queue().await;

        let handle = self.inner.take_handle().await;
        let current = self.inner.session.current_connection().await;
        if let (Some(handle), Some((channel, generation))) = (handle, current) {
            if let Err(status) = subscriber_client(channel)
                .undeclare(pb::UndeclareRequest {
                    client_id: self.inner.session.client_id_owned(),
                    handle,
                })
                .await
            {
                if is_disconnect_status(&status) {
                    self.inner.session.mark_disconnected(generation).await;
                } else if !is_stale_status(&status) {
                    return Err(status.into());
                }
            }
        }
        Ok(())
    }
}

impl QueryableInner {
    fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire) || self.session.is_closed()
    }

    async fn wait_for_connection_or_closed(&self) -> Option<(Channel, u64)> {
        loop {
            if self.is_closed() {
                return None;
            }
            if let Some(connection) = self.session.current_connection().await {
                return Some(connection);
            }
            tokio::select! {
                _ = self.session.inner.connection_notify.notified() => {}
                _ = self.close_notify.notified() => return None,
            }
        }
    }

    async fn ensure_remote_handle(&self, channel: Channel, generation: u64) -> Result<u64, Status> {
        let mut binding = self.binding.lock().await;
        if let Some(handle) = binding.handle {
            return Ok(handle);
        }
        let handle = queryable_client(channel)
            .declare_queryable(pb::DeclareQueryableRequest {
                client_id: self.session.client_id_owned(),
                key_expr: self.args.key_expr.clone(),
                complete: self.args.complete,
                allowed_origin: self.args.allowed_origin,
            })
            .await?
            .into_inner()
            .handle;
        binding.handle = Some(handle);
        binding.generation = generation;
        Ok(handle)
    }

    async fn invalidate_handle(&self, handle: u64) {
        let mut binding = self.binding.lock().await;
        if binding.handle == Some(handle) {
            binding.handle = None;
        }
    }

    async fn take_handle(&self) -> Option<u64> {
        self.binding.lock().await.handle.take()
    }

    async fn event_sender(&self) -> Option<DropOldestSender<QueryableEventEnvelope>> {
        self.event_tx.lock().await.as_ref().cloned()
    }

    async fn close_queue(&self) {
        self.event_tx.lock().await.take();
        self.stream_ready.store(false, Ordering::Release);
    }

    async fn is_query_stale(&self, identity: QueryIdentity) -> bool {
        self.stale_queries.lock().await.contains(&identity)
    }

    async fn mark_query_stale(&self, identity: QueryIdentity) {
        self.active_queries
            .lock()
            .expect("active_queries poisoned")
            .remove(&identity.query_id);
        if self.stale_queries.lock().await.insert(identity) {
            eprintln!(
                "zenoh-grpc-client-rs: query {} is no longer valid on the server; dropping pending replies",
                identity.query_id
            );
        }
    }

    fn remember_query(&self, identity: QueryIdentity) {
        self.active_queries
            .lock()
            .expect("active_queries poisoned")
            .insert(identity.query_id, identity);
    }

    fn forget_query(&self, query_id: u64) {
        self.active_queries
            .lock()
            .expect("active_queries poisoned")
            .remove(&query_id);
    }

    fn query_identity(&self, query_id: u64) -> Option<QueryIdentity> {
        self.active_queries
            .lock()
            .expect("active_queries poisoned")
            .get(&query_id)
            .copied()
    }

    fn mark_stream_ready(&self) {
        self.stream_ready.store(true, Ordering::Release);
        self.ready_notify.notify_waiters();
    }

    fn mark_stream_not_ready(&self) {
        self.stream_ready.store(false, Ordering::Release);
    }

    async fn wait_initial_stream_ready(&self) {
        if self.session.current_connection().await.is_none() {
            return;
        }
        if self.stream_ready.load(Ordering::Acquire) {
            return;
        }
        let _ = tokio::time::timeout(STREAM_READY_TIMEOUT, async {
            while !self.stream_ready.load(Ordering::Acquire) && !self.is_closed() {
                self.ready_notify.notified().await;
            }
        })
        .await;
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

    fn spawn_event_worker(&self) {
        let queryable = self.clone();
        tokio::spawn(async move {
            loop {
                if queryable.inner.is_closed() {
                    break;
                }
                let Some((channel, generation)) = queryable.inner.wait_for_connection_or_closed().await
                else {
                    break;
                };

                let handle = match queryable
                    .inner
                    .ensure_remote_handle(channel.clone(), generation)
                    .await
                {
                    Ok(handle) => handle,
                    Err(status) if is_disconnect_status(&status) => {
                        queryable.inner.session.mark_disconnected(generation).await;
                        continue;
                    }
                    Err(status) => {
                        log_operation_warning("queryable declare", &status);
                        tokio::time::sleep(RECONNECT_DELAY_INITIAL).await;
                        continue;
                    }
                };

                let response = queryable_client(channel.clone())
                    .events(pb::QueryableEventsRequest {
                        client_id: queryable.inner.session.client_id_owned(),
                        handle,
                    })
                    .await;
                let mut stream = match response {
                    Ok(response) => response.into_inner(),
                    Err(status) if is_disconnect_status(&status) => {
                        queryable.inner.session.mark_disconnected(generation).await;
                        continue;
                    }
                    Err(status) if is_stale_status(&status) => {
                        queryable.inner.invalidate_handle(handle).await;
                        continue;
                    }
                    Err(status) => {
                        log_operation_warning("queryable events", &status);
                        tokio::time::sleep(RECONNECT_DELAY_INITIAL).await;
                        continue;
                    }
                };
                queryable.inner.mark_stream_ready();

                loop {
                    let next = tokio::select! {
                        biased;
                        _ = queryable.inner.close_notify.notified() => None,
                        next = stream.message() => Some(next),
                    };
                    let Some(next) = next else {
                        break;
                    };
                    match next {
                        Ok(Some(event)) => {
                            if let Some(tx) = queryable.inner.event_sender().await {
                                let _ = tx.push(QueryableEventEnvelope {
                                    remote_handle: handle,
                                    event,
                                });
                            } else {
                                break;
                            }
                        }
                        Ok(None) => {
                            queryable.inner.mark_stream_not_ready();
                            queryable.inner.session.mark_disconnected(generation).await;
                            break;
                        }
                        Err(status) if is_disconnect_status(&status) => {
                            queryable.inner.mark_stream_not_ready();
                            queryable.inner.session.mark_disconnected(generation).await;
                            break;
                        }
                        Err(status) if is_stale_status(&status) => {
                            queryable.inner.mark_stream_not_ready();
                            queryable.inner.invalidate_handle(handle).await;
                            break;
                        }
                        Err(status) => {
                            queryable.inner.mark_stream_not_ready();
                            log_operation_warning("queryable stream", &status);
                            break;
                        }
                    }
                }
            }
        });
    }

    async fn wait_initial_stream_ready(&self) {
        self.inner.wait_initial_stream_ready().await;
    }

    fn spawn_worker(&self, rx: DropOldestReceiver<QueryableCommand>) {
        let queryable = self.clone();
        tokio::spawn(async move {
            while let Ok(command) = rx.recv_async().await {
                let identity = match &command {
                    QueryableCommand::Reply { identity, .. }
                    | QueryableCommand::ReplyErr { identity, .. }
                    | QueryableCommand::ReplyDelete { identity, .. }
                    | QueryableCommand::Finish { identity } => *identity,
                };

                if queryable.inner.is_query_stale(identity).await {
                    rx.processed_one();
                    continue;
                }

                loop {
                    if queryable.inner.is_closed() {
                        rx.processed_one();
                        break;
                    }
                    let Some((channel, generation)) =
                        queryable.inner.wait_for_connection_or_closed().await
                    else {
                        rx.processed_one();
                        break;
                    };

                    let result = match &command {
                        QueryableCommand::Reply { identity, args } => queryable_client(channel.clone())
                            .reply(pb::QueryReplyRequest {
                                client_id: queryable.inner.session.client_id_owned(),
                                handle: identity.remote_handle,
                                query_id: identity.query_id,
                                key_expr: args.key_expr.clone(),
                                payload: args.payload.clone(),
                                encoding: args.encoding.clone(),
                                attachment: args.attachment.clone(),
                                timestamp: args.timestamp.clone(),
                            })
                            .await
                            .map(|_| ()),
                        QueryableCommand::ReplyErr { identity, args } => queryable_client(channel.clone())
                            .reply_err(pb::QueryReplyErrRequest {
                                client_id: queryable.inner.session.client_id_owned(),
                                handle: identity.remote_handle,
                                query_id: identity.query_id,
                                payload: args.payload.clone(),
                                encoding: args.encoding.clone(),
                            })
                            .await
                            .map(|_| ()),
                        QueryableCommand::ReplyDelete { identity, args } => queryable_client(channel.clone())
                            .reply_delete(pb::QueryReplyDeleteRequest {
                                client_id: queryable.inner.session.client_id_owned(),
                                handle: identity.remote_handle,
                                query_id: identity.query_id,
                                key_expr: args.key_expr.clone(),
                                attachment: args.attachment.clone(),
                                timestamp: args.timestamp.clone(),
                            })
                            .await
                            .map(|_| ()),
                        QueryableCommand::Finish { identity } => queryable_client(channel.clone())
                            .finish(pb::QueryFinishRequest {
                                client_id: queryable.inner.session.client_id_owned(),
                                handle: identity.remote_handle,
                                query_id: identity.query_id,
                            })
                            .await
                            .map(|_| ()),
                    };

                    match result {
                        Ok(()) => {
                            if matches!(command, QueryableCommand::Finish { .. }) {
                                queryable.inner.forget_query(identity.query_id);
                            }
                            rx.processed_one();
                            break;
                        }
                        Err(status) if is_disconnect_status(&status) => {
                            queryable.inner.session.mark_disconnected(generation).await;
                        }
                        Err(status) if is_stale_status(&status) => {
                            queryable.inner.mark_query_stale(identity).await;
                            rx.processed_one();
                            break;
                        }
                        Err(status) => {
                            log_operation_warning("query reply", &status);
                            rx.processed_one();
                            break;
                        }
                    }
                }
            }
        });
    }

    pub fn handle(&self) -> u64 {
        self.inner.local_handle
    }

    pub fn send_dropped_count(&self) -> u64 {
        self.write_tx.dropped_count()
    }

    pub async fn reply(&self, args: QueryReplyArgs) -> Result<(), Error> {
        let identity = self
            .inner
            .query_identity(args.query_id)
            .ok_or_else(|| Status::not_found("query not found"))?;
        self.write_tx
            .push(QueryableCommand::Reply { identity, args })
            .map_err(|_| Status::unavailable("queryable send queue closed").into())
    }

    pub async fn reply_err(&self, args: QueryReplyErrArgs) -> Result<(), Error> {
        let identity = self
            .inner
            .query_identity(args.query_id)
            .ok_or_else(|| Status::not_found("query not found"))?;
        self.write_tx
            .push(QueryableCommand::ReplyErr { identity, args })
            .map_err(|_| Status::unavailable("queryable send queue closed").into())
    }

    pub async fn reply_delete(&self, args: QueryReplyDeleteArgs) -> Result<(), Error> {
        let identity = self
            .inner
            .query_identity(args.query_id)
            .ok_or_else(|| Status::not_found("query not found"))?;
        self.write_tx
            .push(QueryableCommand::ReplyDelete { identity, args })
            .map_err(|_| Status::unavailable("queryable send queue closed").into())
    }

    pub fn recv(&self) -> Result<GrpcQuery, Error> {
        self.ensure_pull_mode()?;
        let event = self
            .rx
            .recv()
            .map_err(|_| Status::unavailable("query stream closed"))?;
        self.query_from_event(event)
    }

    pub fn try_recv(&self) -> Result<Option<GrpcQuery>, Error> {
        self.ensure_pull_mode()?;
        match self.rx.try_recv() {
            Ok(event) => Ok(Some(self.query_from_event(event)?)),
            Err(flume::TryRecvError::Empty) => Ok(None),
            Err(flume::TryRecvError::Disconnected) => {
                Err(Status::unavailable("query stream closed").into())
            }
        }
    }

    pub async fn recv_async(&self) -> Result<GrpcQuery, Error> {
        self.ensure_pull_mode()?;
        let event = self
            .rx
            .recv_async()
            .await
            .map_err(|_| Status::unavailable("query stream closed"))?;
        self.query_from_event(event)
    }

    pub fn dropped_count(&self) -> Result<u64, Error> {
        self.ensure_pull_mode()?;
        Ok(self.rx.dropped_count())
    }

    pub fn is_closed(&self) -> Result<bool, Error> {
        self.ensure_pull_mode()?;
        Ok(self.inner.is_closed() && self.rx.is_disconnected())
    }

    pub async fn undeclare(&self) -> Result<(), Error> {
        self.inner.close_queue().await;
        self.write_tx.wait_empty().await;
        self.inner.closed.store(true, Ordering::Release);
        self.inner.close_notify.notify_waiters();

        let handle = self.inner.take_handle().await;
        let current = self.inner.session.current_connection().await;
        if let (Some(handle), Some((channel, generation))) = (handle, current) {
            if let Err(status) = queryable_client(channel)
                .undeclare(pb::UndeclareRequest {
                    client_id: self.inner.session.client_id_owned(),
                    handle,
                })
                .await
            {
                if is_disconnect_status(&status) {
                    self.inner.session.mark_disconnected(generation).await;
                } else if !is_stale_status(&status) {
                    return Err(status.into());
                }
            }
        }
        Ok(())
    }

    fn query_from_event(&self, event: QueryableEventEnvelope) -> Result<GrpcQuery, Error> {
        let inner = event
            .event
            .query
            .ok_or_else(|| Status::unavailable("queryable event missing query"))?;
        let identity = QueryIdentity {
            remote_handle: event.remote_handle,
            query_id: inner.query_id,
        };
        self.inner.remember_query(identity);
        Ok(GrpcQuery {
            queryable: self.clone(),
            inner,
            remote_handle: identity.remote_handle,
            finished: Arc::new(AtomicBool::new(false)),
        })
    }
}

impl QuerierInner {
    fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire) || self.session.is_closed()
    }

    async fn wait_for_connection_or_closed(&self) -> Option<(Channel, u64)> {
        loop {
            if self.is_closed() {
                return None;
            }
            if let Some(connection) = self.session.current_connection().await {
                return Some(connection);
            }
            tokio::select! {
                _ = self.session.inner.connection_notify.notified() => {}
                _ = self.close_notify.notified() => return None,
            }
        }
    }

    async fn ensure_remote_handle(&self, channel: Channel, generation: u64) -> Result<u64, Status> {
        let mut binding = self.binding.lock().await;
        if let Some(handle) = binding.handle {
            return Ok(handle);
        }
        let handle = querier_client(channel)
            .declare_querier(pb::DeclareQuerierRequest {
                client_id: self.session.client_id_owned(),
                key_expr: self.args.key_expr.clone(),
                target: self.args.target,
                consolidation: self.args.consolidation,
                timeout_ms: self.args.timeout_ms,
                allowed_destination: self.args.allowed_destination,
            })
            .await?
            .into_inner()
            .handle;
        binding.handle = Some(handle);
        binding.generation = generation;
        Ok(handle)
    }

    async fn invalidate_handle(&self, handle: u64) {
        let mut binding = self.binding.lock().await;
        if binding.handle == Some(handle) {
            binding.handle = None;
        }
    }

    async fn take_handle(&self) -> Option<u64> {
        self.binding.lock().await.handle.take()
    }
}

impl GrpcQuerier {
    pub fn handle(&self) -> u64 {
        self.inner.local_handle
    }

    pub async fn get(&self, req: QuerierGetArgs) -> Result<ReplyStream, Error> {
        let Some((channel, generation)) = self.inner.wait_for_connection_or_closed().await else {
            self.inner.session.warn_disconnected_once();
            return Ok(empty_reply_stream());
        };

        let handle = match self.inner.ensure_remote_handle(channel.clone(), generation).await {
            Ok(handle) => handle,
            Err(status) if is_disconnect_status(&status) => {
                self.inner.session.mark_disconnected(generation).await;
                return Ok(empty_reply_stream());
            }
            Err(status) => return Err(status.into()),
        };

        let response = querier_client(channel.clone())
            .get(pb::QuerierGetRequest {
                client_id: self.inner.session.client_id_owned(),
                handle,
                parameters: req.parameters,
                payload: req.payload,
                encoding: req.encoding,
                attachment: req.attachment,
            })
            .await;
        let mut stream = match response {
            Ok(response) => response.into_inner(),
            Err(status) if is_disconnect_status(&status) => {
                self.inner.session.mark_disconnected(generation).await;
                return Ok(empty_reply_stream());
            }
            Err(status) if is_stale_status(&status) => {
                self.inner.invalidate_handle(handle).await;
                return Ok(empty_reply_stream());
            }
            Err(status) => return Err(status.into()),
        };

        let (tx, rx) = bounded_drop_oldest(RECV_QUEUE_CAPACITY);
        let session = self.inner.session.clone();
        tokio::spawn(async move {
            loop {
                match stream.message().await {
                    Ok(Some(reply)) => {
                        let _ = tx.push(reply);
                    }
                    Ok(None) => break,
                    Err(status) => {
                        if is_disconnect_status(&status) {
                            session.mark_disconnected(generation).await;
                        }
                        break;
                    }
                }
            }
        });
        Ok(ReplyStream { rx })
    }

    pub async fn undeclare(&self) -> Result<(), Error> {
        self.inner.closed.store(true, Ordering::Release);
        self.inner.close_notify.notify_waiters();
        let handle = self.inner.take_handle().await;
        let current = self.inner.session.current_connection().await;
        if let (Some(handle), Some((channel, generation))) = (handle, current) {
            if let Err(status) = querier_client(channel)
                .undeclare(pb::UndeclareRequest {
                    client_id: self.inner.session.client_id_owned(),
                    handle,
                })
                .await
            {
                if is_disconnect_status(&status) {
                    self.inner.session.mark_disconnected(generation).await;
                } else if !is_stale_status(&status) {
                    return Err(status.into());
                }
            }
        }
        Ok(())
    }
}

impl ReplyStream {
    pub fn recv(&self) -> Result<pb::Reply, Error> {
        self.rx
            .recv()
            .map_err(|_| Status::unavailable("reply stream closed").into())
    }

    pub fn try_recv(&self) -> Result<Option<pb::Reply>, Error> {
        match self.rx.try_recv() {
            Ok(reply) => Ok(Some(reply)),
            Err(flume::TryRecvError::Empty) => Ok(None),
            Err(flume::TryRecvError::Disconnected) => {
                Err(Status::unavailable("reply stream closed").into())
            }
        }
    }

    pub async fn recv_async(&self) -> Result<pb::Reply, Error> {
        self.rx
            .recv_async()
            .await
            .map_err(|_| Status::unavailable("reply stream closed").into())
    }

    pub fn dropped_count(&self) -> u64 {
        self.rx.dropped_count()
    }

    pub fn is_closed(&self) -> bool {
        self.rx.is_disconnected()
    }
}

impl GrpcQuery {
    fn identity(&self) -> QueryIdentity {
        QueryIdentity {
            remote_handle: self.remote_handle,
            query_id: self.inner.query_id,
        }
    }

    fn enqueue_command(&self, command: QueryableCommand) -> Result<(), Error> {
        self.queryable
            .write_tx
            .push(command)
            .map_err(|_| Status::unavailable("queryable send queue closed").into())
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
        let args = QueryReplyArgs {
            query_id: self.inner.query_id,
            key_expr: key_expr.into(),
            payload: payload.into(),
            encoding: encoding.into(),
            attachment: attachment.into(),
            timestamp: timestamp.into(),
        };
        self.enqueue_command(QueryableCommand::Reply {
            identity: self.identity(),
            args,
        })
    }

    pub async fn reply_err(
        &self,
        payload: impl Into<Vec<u8>>,
        encoding: impl Into<String>,
    ) -> Result<(), Error> {
        let args = QueryReplyErrArgs {
            query_id: self.inner.query_id,
            payload: payload.into(),
            encoding: encoding.into(),
        };
        self.enqueue_command(QueryableCommand::ReplyErr {
            identity: self.identity(),
            args,
        })
    }

    pub async fn reply_delete(
        &self,
        key_expr: impl Into<String>,
        attachment: impl Into<Vec<u8>>,
        timestamp: impl Into<String>,
    ) -> Result<(), Error> {
        let args = QueryReplyDeleteArgs {
            query_id: self.inner.query_id,
            key_expr: key_expr.into(),
            attachment: attachment.into(),
            timestamp: timestamp.into(),
        };
        self.enqueue_command(QueryableCommand::ReplyDelete {
            identity: self.identity(),
            args,
        })
    }

    pub async fn finish(self) -> Result<(), Error> {
        if self.mark_finished() {
            self.enqueue_command(QueryableCommand::Finish {
                identity: self.identity(),
            })?;
        }
        Ok(())
    }
}

impl Drop for GrpcQuery {
    fn drop(&mut self) {
        if self.mark_finished() {
            let _ = self.enqueue_command(QueryableCommand::Finish {
                identity: self.identity(),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unreachable_addr() -> ConnectAddr {
        ConnectAddr::Tcp("127.0.0.1:65534".into())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn connect_and_declare_succeed_while_offline() {
        let session = GrpcSession::connect(unreachable_addr()).await.unwrap();

        let publisher = session
            .declare_publisher(DeclarePublisherArgs {
                key_expr: "demo/offline/pub".into(),
                ..Default::default()
            })
            .await
            .unwrap();
        let subscriber = session
            .declare_subscriber(
                DeclareSubscriberArgs {
                    key_expr: "demo/offline/**".into(),
                    ..Default::default()
                },
                None,
            )
            .await
            .unwrap();
        let queryable = session
            .declare_queryable(
                DeclareQueryableArgs {
                    key_expr: "demo/offline/query/**".into(),
                    ..Default::default()
                },
                None,
            )
            .await
            .unwrap();
        let querier = session
            .declare_querier(DeclareQuerierArgs {
                key_expr: "demo/offline/query/**".into(),
                ..Default::default()
            })
            .await
            .unwrap();

        assert_ne!(publisher.handle(), 0);
        assert_ne!(subscriber.handle(), 0);
        assert_ne!(queryable.handle(), 0);
        assert_ne!(querier.handle(), 0);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn offline_get_returns_closed_empty_stream() {
        let session = GrpcSession::connect(unreachable_addr()).await.unwrap();

        let replies = session
            .get(SessionGetArgs {
                selector: "demo/offline/get".into(),
                ..Default::default()
            })
            .await
            .unwrap();

        assert!(replies.is_closed());
        assert!(replies.recv_async().await.is_err());
    }
}
