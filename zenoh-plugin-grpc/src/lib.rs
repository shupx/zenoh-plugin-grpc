mod config;

use std::{
    collections::HashMap,
    pin::Pin,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

use async_stream::try_stream;
use config::{Config, ListenEndpoint};
use futures_core::Stream;
use tokio::{
    net::{TcpListener, UnixListener},
    sync::{broadcast, Mutex, Notify, RwLock},
    task::JoinHandle,
};
use tokio_stream::{wrappers::BroadcastStream, StreamExt};
use tonic::{transport::Server, Request, Response, Status};
use tracing::{info, warn};
use zenoh::{
    bytes::{Encoding, ZBytes},
    internal::{
        plugins::{RunningPluginTrait, ZenohPlugin},
        runtime::DynamicRuntime,
    },
    qos::{CongestionControl, Priority},
    query::{ConsolidationMode, QueryConsolidation, QueryTarget, Reply},
    sample::{Locality, Sample, SampleKind},
};
use zenoh_grpc_proto::v1::{
    self as pb,
    publisher_service_server::{PublisherService, PublisherServiceServer},
    querier_service_server::{QuerierService, QuerierServiceServer},
    queryable_service_server::{QueryableService, QueryableServiceServer},
    session_service_server::{SessionService, SessionServiceServer},
    subscriber_service_server::{SubscriberService, SubscriberServiceServer},
};
use zenoh_plugin_trait::{plugin_long_version, plugin_version, Plugin, PluginControl};
use zenoh_result::{zerror, ZResult};

type BoxReplyStream = Pin<Box<dyn Stream<Item = Result<pb::Reply, Status>> + Send + 'static>>;
type BoxSubscriberStream =
    Pin<Box<dyn Stream<Item = Result<pb::SubscriberEvent, Status>> + Send + 'static>>;
type BoxQueryableStream =
    Pin<Box<dyn Stream<Item = Result<pb::QueryableEvent, Status>> + Send + 'static>>;

lazy_static::lazy_static! {
    static ref TOKIO_RUNTIME: tokio::runtime::Runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .max_blocking_threads(50)
        .enable_all()
        .build()
        .expect("Unable to create runtime");
}

#[inline(always)]
fn spawn_runtime<F>(task: F) -> JoinHandle<F::Output>
where
    F: std::future::Future + Send + 'static,
    F::Output: Send + 'static,
{
    match tokio::runtime::Handle::try_current() {
        Ok(rt) => rt.spawn(task),
        Err(_) => TOKIO_RUNTIME.spawn(task),
    }
}

pub struct GrpcPlugin;

#[cfg(feature = "dynamic_plugin")]
zenoh_plugin_trait::declare_plugin!(GrpcPlugin);

impl ZenohPlugin for GrpcPlugin {}

impl Plugin for GrpcPlugin {
    type StartArgs = DynamicRuntime;
    type Instance = zenoh::internal::plugins::RunningPlugin;

    const DEFAULT_NAME: &'static str = "grpc";
    const PLUGIN_VERSION: &'static str = plugin_version!();
    const PLUGIN_LONG_VERSION: &'static str = plugin_long_version!();

    fn start(name: &str, runtime: &Self::StartArgs) -> ZResult<Self::Instance> {
        zenoh_util::try_init_log_from_env();
        let plugin_conf = runtime
            .get_config()
            .get_plugin_config(name)
            .map_err(|_| zerror!("Plugin `{name}`: missing config"))?;
        let config: Config = serde_json::from_value(plugin_conf)
            .map_err(|e| zerror!("Plugin `{name}` configuration error: {e}"))?;

        let stop = Arc::new(Notify::new());
        let running = RunningPlugin {
            stop: stop.clone(),
            _task: spawn_runtime(run(runtime.clone(), config, stop)),
        };
        Ok(Box::new(running))
    }
}

struct RunningPlugin {
    stop: Arc<Notify>,
    _task: JoinHandle<()>,
}

impl PluginControl for RunningPlugin {}
impl RunningPluginTrait for RunningPlugin {}

impl Drop for RunningPlugin {
    fn drop(&mut self) {
        self.stop.notify_waiters();
    }
}

struct PublisherEntry {
    owner: String,
    publisher: zenoh::pubsub::Publisher<'static>,
}

struct SubscriberEntry {
    owner: String,
    subscriber: zenoh::pubsub::Subscriber<flume::Receiver<Sample>>,
    events: broadcast::Sender<pb::SubscriberEvent>,
}

struct QueryableEntry {
    owner: String,
    queryable: zenoh::query::Queryable<flume::Receiver<zenoh::query::Query>>,
    events: broadcast::Sender<pb::QueryableEvent>,
}

struct QuerierEntry {
    owner: String,
    querier: zenoh::query::Querier<'static>,
}

struct InFlightQuery {
    owner: String,
    handle: u64,
    query: zenoh::query::Query,
}

struct PluginState {
    session: zenoh::Session,
    next_handle: AtomicU64,
    next_query_id: AtomicU64,
    publishers: RwLock<HashMap<u64, PublisherEntry>>,
    subscribers: RwLock<HashMap<u64, SubscriberEntry>>,
    queryables: RwLock<HashMap<u64, QueryableEntry>>,
    queriers: RwLock<HashMap<u64, QuerierEntry>>,
    inflight_queries: Mutex<HashMap<u64, InFlightQuery>>,
    config: Config,
}

impl PluginState {
    fn new(session: zenoh::Session, config: Config) -> Self {
        Self {
            session,
            next_handle: AtomicU64::new(1),
            next_query_id: AtomicU64::new(1),
            publishers: RwLock::new(HashMap::new()),
            subscribers: RwLock::new(HashMap::new()),
            queryables: RwLock::new(HashMap::new()),
            queriers: RwLock::new(HashMap::new()),
            inflight_queries: Mutex::new(HashMap::new()),
            config,
        }
    }

    fn alloc_handle(&self) -> u64 {
        self.next_handle.fetch_add(1, Ordering::Relaxed)
    }

    fn alloc_query_id(&self) -> u64 {
        self.next_query_id.fetch_add(1, Ordering::Relaxed)
    }

    async fn cleanup_client(&self, client_id: &str) {
        let publisher_handles: Vec<u64> = self
            .publishers
            .read()
            .await
            .iter()
            .filter_map(|(handle, entry)| (entry.owner == client_id).then_some(*handle))
            .collect();
        for handle in publisher_handles {
            let entry = self.publishers.write().await.remove(&handle);
            if let Some(entry) = entry {
                let _ = entry.publisher.undeclare().await;
            }
        }

        let subscriber_handles: Vec<u64> = self
            .subscribers
            .read()
            .await
            .iter()
            .filter_map(|(handle, entry)| (entry.owner == client_id).then_some(*handle))
            .collect();
        for handle in subscriber_handles {
            let entry = self.subscribers.write().await.remove(&handle);
            if let Some(entry) = entry {
                let _ = entry.subscriber.undeclare().await;
            }
        }

        let queryable_handles: Vec<u64> = self
            .queryables
            .read()
            .await
            .iter()
            .filter_map(|(handle, entry)| (entry.owner == client_id).then_some(*handle))
            .collect();
        for handle in queryable_handles {
            let entry = self.queryables.write().await.remove(&handle);
            if let Some(entry) = entry {
                let _ = entry.queryable.undeclare().await;
            }
        }

        let querier_handles: Vec<u64> = self
            .queriers
            .read()
            .await
            .iter()
            .filter_map(|(handle, entry)| (entry.owner == client_id).then_some(*handle))
            .collect();
        for handle in querier_handles {
            let entry = self.queriers.write().await.remove(&handle);
            if let Some(entry) = entry {
                let _ = entry.querier.undeclare().await;
            }
        }

        self.inflight_queries
            .lock()
            .await
            .retain(|_, query| query.owner != client_id);
    }
}

#[derive(Clone)]
struct GrpcServer {
    state: Arc<PluginState>,
}

impl GrpcServer {
    fn new(state: Arc<PluginState>) -> Self {
        Self { state }
    }
}

fn ensure_owner(client_id: &str, owner: &str) -> Result<(), Status> {
    if client_id != owner {
        Err(Status::permission_denied("handle belongs to another client"))
    } else {
        Ok(())
    }
}

fn map_locality(locality: i32) -> Locality {
    match pb::Locality::try_from(locality).unwrap_or(pb::Locality::Unspecified) {
        pb::Locality::SessionLocal => Locality::SessionLocal,
        pb::Locality::Remote => Locality::Remote,
        _ => Locality::Any,
    }
}

fn map_congestion_control(cc: i32) -> CongestionControl {
    match pb::CongestionControl::try_from(cc).unwrap_or(pb::CongestionControl::Unspecified) {
        pb::CongestionControl::Drop => CongestionControl::Drop,
        _ => CongestionControl::Block,
    }
}

fn map_priority(priority: i32) -> Priority {
    match pb::Priority::try_from(priority).unwrap_or(pb::Priority::Unspecified) {
        pb::Priority::RealTime => Priority::RealTime,
        pb::Priority::InteractiveHigh => Priority::InteractiveHigh,
        pb::Priority::InteractiveLow => Priority::InteractiveLow,
        pb::Priority::DataHigh => Priority::DataHigh,
        pb::Priority::DataLow => Priority::DataLow,
        pb::Priority::Background => Priority::Background,
        _ => Priority::Data,
    }
}

fn map_reliability(reliability: i32) -> zenoh::qos::Reliability {
    match pb::Reliability::try_from(reliability).unwrap_or(pb::Reliability::Unspecified) {
        pb::Reliability::Reliable => zenoh::qos::Reliability::Reliable,
        _ => zenoh::qos::Reliability::BestEffort,
    }
}

fn map_query_target(target: i32) -> QueryTarget {
    match pb::QueryTarget::try_from(target).unwrap_or(pb::QueryTarget::Unspecified) {
        pb::QueryTarget::All => QueryTarget::All,
        pb::QueryTarget::AllComplete => QueryTarget::AllComplete,
        _ => QueryTarget::BestMatching,
    }
}

fn map_consolidation(mode: i32) -> QueryConsolidation {
    match pb::ConsolidationMode::try_from(mode).unwrap_or(pb::ConsolidationMode::Unspecified) {
        pb::ConsolidationMode::None => ConsolidationMode::None.into(),
        pb::ConsolidationMode::Monotonic => ConsolidationMode::Monotonic.into(),
        pb::ConsolidationMode::Latest => ConsolidationMode::Latest.into(),
        _ => QueryConsolidation::AUTO,
    }
}

fn opt_attachment(bytes: Vec<u8>) -> Option<ZBytes> {
    (!bytes.is_empty()).then(|| bytes.into())
}

fn sample_kind(kind: SampleKind) -> i32 {
    match kind {
        SampleKind::Delete => pb::SampleKind::Delete as i32,
        _ => pb::SampleKind::Put as i32,
    }
}

fn encode_sample(sample: &Sample) -> pb::Sample {
    pb::Sample {
        key_expr: sample.key_expr().to_string(),
        payload: sample.payload().to_bytes().into_owned(),
        encoding: String::from(sample.encoding().clone()),
        kind: sample_kind(sample.kind()),
        attachment: sample
            .attachment()
            .map(|a| a.to_bytes().into_owned())
            .unwrap_or_default(),
        timestamp: sample.timestamp().map(|ts| ts.to_string()).unwrap_or_default(),
        source_info: None,
    }
}

fn encode_reply(reply: Reply) -> pb::Reply {
    match reply.result() {
        Ok(sample) => pb::Reply {
            result: Some(pb::reply::Result::Sample(encode_sample(sample))),
        },
        Err(err) => pb::Reply {
            result: Some(pb::reply::Result::Error(pb::ReplyError {
                payload: err.payload().to_bytes().into_owned(),
                encoding: String::from(err.encoding().clone()),
            })),
        },
    }
}

#[tonic::async_trait]
impl SessionService for GrpcServer {
    async fn info(
        &self,
        _request: Request<pb::SessionInfoRequest>,
    ) -> Result<Response<pb::SessionInfoReply>, Status> {
        Ok(Response::new(pb::SessionInfoReply {
            zid: self.state.session.zid().to_string(),
        }))
    }

    async fn put(
        &self,
        request: Request<pb::SessionPutRequest>,
    ) -> Result<Response<pb::Empty>, Status> {
        let req = request.into_inner();
        let mut builder = self.state.session.put(req.key_expr, req.payload);
        builder = builder
            .encoding(Encoding::from(req.encoding))
            .congestion_control(map_congestion_control(req.congestion_control))
            .priority(map_priority(req.priority))
            .express(req.express)
            .allowed_destination(map_locality(req.allowed_destination));
        if let Some(attachment) = opt_attachment(req.attachment) {
            builder = builder.attachment(attachment);
        }
        builder.await.map_err(internal_status)?;
        Ok(Response::new(pb::Empty {}))
    }

    async fn delete(
        &self,
        request: Request<pb::SessionDeleteRequest>,
    ) -> Result<Response<pb::Empty>, Status> {
        let req = request.into_inner();
        let mut builder = self.state.session.delete(req.key_expr);
        builder = builder
            .congestion_control(map_congestion_control(req.congestion_control))
            .priority(map_priority(req.priority))
            .express(req.express)
            .allowed_destination(map_locality(req.allowed_destination));
        if let Some(attachment) = opt_attachment(req.attachment) {
            builder = builder.attachment(attachment);
        }
        builder.await.map_err(internal_status)?;
        Ok(Response::new(pb::Empty {}))
    }

    type GetStream = BoxReplyStream;

    async fn get(
        &self,
        request: Request<pb::SessionGetRequest>,
    ) -> Result<Response<Self::GetStream>, Status> {
        let req = request.into_inner();
        let mut builder = self.state.session.get(req.selector);
        builder = builder
            .target(map_query_target(req.target))
            .consolidation(map_consolidation(req.consolidation))
            .allowed_destination(map_locality(req.allowed_destination));
        if req.timeout_ms != 0 {
            builder = builder.timeout(Duration::from_millis(req.timeout_ms));
        }
        if !req.payload.is_empty() {
            builder = builder.payload(req.payload).encoding(Encoding::from(req.encoding));
        }
        if let Some(attachment) = opt_attachment(req.attachment) {
            builder = builder.attachment(attachment);
        }
        let replies = builder.await.map_err(internal_status)?;
        let stream = try_stream! {
            while let Ok(reply) = replies.recv_async().await {
                yield encode_reply(reply);
            }
        };
        Ok(Response::new(Box::pin(stream)))
    }

    async fn cleanup_client(
        &self,
        request: Request<pb::CleanupClientRequest>,
    ) -> Result<Response<pb::Empty>, Status> {
        self.state.cleanup_client(&request.into_inner().client_id).await;
        Ok(Response::new(pb::Empty {}))
    }
}

#[tonic::async_trait]
impl PublisherService for GrpcServer {
    async fn declare_publisher(
        &self,
        request: Request<pb::DeclarePublisherRequest>,
    ) -> Result<Response<pb::HandleReply>, Status> {
        let req = request.into_inner();
        let handle = self.state.alloc_handle();
        let publisher = self
            .state
            .session
            .declare_publisher(req.key_expr)
            .encoding(Encoding::from(req.encoding))
            .congestion_control(map_congestion_control(req.congestion_control))
            .priority(map_priority(req.priority))
            .express(req.express)
            .reliability(map_reliability(req.reliability))
            .allowed_destination(map_locality(req.allowed_destination))
            .await
            .map_err(internal_status)?;
        self.state.publishers.write().await.insert(
            handle,
            PublisherEntry {
                owner: req.client_id,
                publisher,
            },
        );
        Ok(Response::new(pb::HandleReply { handle }))
    }

    async fn put(
        &self,
        request: Request<pb::PublisherPutRequest>,
    ) -> Result<Response<pb::Empty>, Status> {
        let req = request.into_inner();
        let publishers = self.state.publishers.read().await;
        let entry = publishers
            .get(&req.handle)
            .ok_or_else(|| Status::not_found("publisher handle not found"))?;
        ensure_owner(&req.client_id, &entry.owner)?;
        let mut builder = entry.publisher.put(req.payload);
        if !req.encoding.is_empty() {
            builder = builder.encoding(Encoding::from(req.encoding));
        }
        if let Some(attachment) = opt_attachment(req.attachment) {
            builder = builder.attachment(attachment);
        }
        builder.await.map_err(internal_status)?;
        Ok(Response::new(pb::Empty {}))
    }

    async fn delete(
        &self,
        request: Request<pb::PublisherDeleteRequest>,
    ) -> Result<Response<pb::Empty>, Status> {
        let req = request.into_inner();
        let publishers = self.state.publishers.read().await;
        let entry = publishers
            .get(&req.handle)
            .ok_or_else(|| Status::not_found("publisher handle not found"))?;
        ensure_owner(&req.client_id, &entry.owner)?;
        let mut builder = entry.publisher.delete();
        if let Some(attachment) = opt_attachment(req.attachment) {
            builder = builder.attachment(attachment);
        }
        builder.await.map_err(internal_status)?;
        Ok(Response::new(pb::Empty {}))
    }

    async fn undeclare(
        &self,
        request: Request<pb::UndeclareRequest>,
    ) -> Result<Response<pb::Empty>, Status> {
        let req = request.into_inner();
        let entry = self
            .state
            .publishers
            .write()
            .await
            .remove(&req.handle)
            .ok_or_else(|| Status::not_found("publisher handle not found"))?;
        ensure_owner(&req.client_id, &entry.owner)?;
        entry.publisher.undeclare().await.map_err(internal_status)?;
        Ok(Response::new(pb::Empty {}))
    }
}

#[tonic::async_trait]
impl SubscriberService for GrpcServer {
    async fn declare_subscriber(
        &self,
        request: Request<pb::DeclareSubscriberRequest>,
    ) -> Result<Response<pb::HandleReply>, Status> {
        let req = request.into_inner();
        let handle = self.state.alloc_handle();
        let subscriber = self
            .state
            .session
            .declare_subscriber(req.key_expr)
            .allowed_origin(map_locality(req.allowed_origin))
            .with(flume::bounded(256))
            .await
            .map_err(internal_status)?;
        let (tx, _) = broadcast::channel(256);
        let tx_bg = tx.clone();
        let receiver = subscriber.handler().clone();
        spawn_runtime(async move {
            while let Ok(sample) = receiver.recv_async().await {
                let _ = tx_bg.send(pb::SubscriberEvent {
                    sample: Some(encode_sample(&sample)),
                });
            }
        });
        self.state.subscribers.write().await.insert(
            handle,
            SubscriberEntry {
                owner: req.client_id,
                subscriber,
                events: tx,
            },
        );
        Ok(Response::new(pb::HandleReply { handle }))
    }

    type EventsStream = BoxSubscriberStream;

    async fn events(
        &self,
        request: Request<pb::SubscriberEventsRequest>,
    ) -> Result<Response<Self::EventsStream>, Status> {
        let req = request.into_inner();
        let subscribers = self.state.subscribers.read().await;
        let entry = subscribers
            .get(&req.handle)
            .ok_or_else(|| Status::not_found("subscriber handle not found"))?;
        ensure_owner(&req.client_id, &entry.owner)?;
        let stream = BroadcastStream::new(entry.events.subscribe())
            .filter_map(|item| item.ok().map(Ok));
        Ok(Response::new(Box::pin(stream)))
    }

    async fn undeclare(
        &self,
        request: Request<pb::UndeclareRequest>,
    ) -> Result<Response<pb::Empty>, Status> {
        let req = request.into_inner();
        let entry = self
            .state
            .subscribers
            .write()
            .await
            .remove(&req.handle)
            .ok_or_else(|| Status::not_found("subscriber handle not found"))?;
        ensure_owner(&req.client_id, &entry.owner)?;
        entry.subscriber.undeclare().await.map_err(internal_status)?;
        Ok(Response::new(pb::Empty {}))
    }
}

#[tonic::async_trait]
impl QueryableService for GrpcServer {
    async fn declare_queryable(
        &self,
        request: Request<pb::DeclareQueryableRequest>,
    ) -> Result<Response<pb::HandleReply>, Status> {
        let req = request.into_inner();
        let handle = self.state.alloc_handle();
        let queryable = self
            .state
            .session
            .declare_queryable(req.key_expr)
            .complete(req.complete)
            .allowed_origin(map_locality(req.allowed_origin))
            .with(flume::bounded(256))
            .await
            .map_err(internal_status)?;
        let (tx, _) = broadcast::channel(256);
        let tx_bg = tx.clone();
        let receiver = queryable.handler().clone();
        let state = self.state.clone();
        let owner = req.client_id.clone();
        spawn_runtime(async move {
            while let Ok(query) = receiver.recv_async().await {
                let query_id = state.alloc_query_id();
                state.inflight_queries.lock().await.insert(
                    query_id,
                    InFlightQuery {
                        owner: owner.clone(),
                        handle,
                        query,
                    },
                );
                if let Some(stored) = state.inflight_queries.lock().await.get(&query_id) {
                    let event = pb::QueryableEvent {
                        query: Some(pb::Query {
                            query_id,
                            selector: stored.query.selector().to_string(),
                            key_expr: stored.query.key_expr().to_string(),
                            parameters: stored.query.parameters().to_string(),
                            payload: stored
                                .query
                                .payload()
                                .map(|p| p.to_bytes().into_owned())
                                .unwrap_or_default(),
                            encoding: stored
                                .query
                                .encoding()
                                .map(|e| String::from(e.clone()))
                                .unwrap_or_default(),
                            attachment: stored
                                .query
                                .attachment()
                                .map(|a| a.to_bytes().into_owned())
                                .unwrap_or_default(),
                        }),
                    };
                    let _ = tx_bg.send(event);
                }
            }
        });
        self.state.queryables.write().await.insert(
            handle,
            QueryableEntry {
                owner: req.client_id,
                queryable,
                events: tx,
            },
        );
        Ok(Response::new(pb::HandleReply { handle }))
    }

    type EventsStream = BoxQueryableStream;

    async fn events(
        &self,
        request: Request<pb::QueryableEventsRequest>,
    ) -> Result<Response<Self::EventsStream>, Status> {
        let req = request.into_inner();
        let queryables = self.state.queryables.read().await;
        let entry = queryables
            .get(&req.handle)
            .ok_or_else(|| Status::not_found("queryable handle not found"))?;
        ensure_owner(&req.client_id, &entry.owner)?;
        let stream = BroadcastStream::new(entry.events.subscribe())
            .filter_map(|item| item.ok().map(Ok));
        Ok(Response::new(Box::pin(stream)))
    }

    async fn reply(
        &self,
        request: Request<pb::QueryReplyRequest>,
    ) -> Result<Response<pb::Empty>, Status> {
        let req = request.into_inner();
        let mut inflight = self.state.inflight_queries.lock().await;
        let query = inflight
            .get_mut(&req.query_id)
            .ok_or_else(|| Status::not_found("query not found"))?;
        ensure_owner(&req.client_id, &query.owner)?;
        if query.handle != req.handle {
            return Err(Status::permission_denied("query does not belong to the queryable handle"));
        }
        let mut builder = query.query.reply(req.key_expr, req.payload);
        if !req.encoding.is_empty() {
            builder = builder.encoding(Encoding::from(req.encoding));
        }
        if let Some(attachment) = opt_attachment(req.attachment) {
            builder = builder.attachment(attachment);
        }
        builder.await.map_err(internal_status)?;
        Ok(Response::new(pb::Empty {}))
    }

    async fn reply_err(
        &self,
        request: Request<pb::QueryReplyErrRequest>,
    ) -> Result<Response<pb::Empty>, Status> {
        let req = request.into_inner();
        let mut inflight = self.state.inflight_queries.lock().await;
        let query = inflight
            .get_mut(&req.query_id)
            .ok_or_else(|| Status::not_found("query not found"))?;
        ensure_owner(&req.client_id, &query.owner)?;
        if query.handle != req.handle {
            return Err(Status::permission_denied("query does not belong to the queryable handle"));
        }
        let mut builder = query.query.reply_err(req.payload);
        if !req.encoding.is_empty() {
            builder = builder.encoding(Encoding::from(req.encoding));
        }
        builder.await.map_err(internal_status)?;
        Ok(Response::new(pb::Empty {}))
    }

    async fn reply_delete(
        &self,
        request: Request<pb::QueryReplyDeleteRequest>,
    ) -> Result<Response<pb::Empty>, Status> {
        let req = request.into_inner();
        let mut inflight = self.state.inflight_queries.lock().await;
        let query = inflight
            .get_mut(&req.query_id)
            .ok_or_else(|| Status::not_found("query not found"))?;
        ensure_owner(&req.client_id, &query.owner)?;
        if query.handle != req.handle {
            return Err(Status::permission_denied("query does not belong to the queryable handle"));
        }
        let mut builder = query.query.reply_del(req.key_expr);
        if let Some(attachment) = opt_attachment(req.attachment) {
            builder = builder.attachment(attachment);
        }
        builder.await.map_err(internal_status)?;
        Ok(Response::new(pb::Empty {}))
    }

    async fn undeclare(
        &self,
        request: Request<pb::UndeclareRequest>,
    ) -> Result<Response<pb::Empty>, Status> {
        let req = request.into_inner();
        let entry = self
            .state
            .queryables
            .write()
            .await
            .remove(&req.handle)
            .ok_or_else(|| Status::not_found("queryable handle not found"))?;
        ensure_owner(&req.client_id, &entry.owner)?;
        self.state
            .inflight_queries
            .lock()
            .await
            .retain(|_, query| query.handle != req.handle);
        entry.queryable.undeclare().await.map_err(internal_status)?;
        Ok(Response::new(pb::Empty {}))
    }
}

#[tonic::async_trait]
impl QuerierService for GrpcServer {
    async fn declare_querier(
        &self,
        request: Request<pb::DeclareQuerierRequest>,
    ) -> Result<Response<pb::HandleReply>, Status> {
        let req = request.into_inner();
        let handle = self.state.alloc_handle();
        let querier = self
            .state
            .session
            .declare_querier(req.key_expr)
            .target(map_query_target(req.target))
            .consolidation(map_consolidation(req.consolidation))
            .allowed_destination(map_locality(req.allowed_destination))
            ;
        let querier = if req.timeout_ms != 0 {
            querier.timeout(Duration::from_millis(req.timeout_ms))
        } else {
            querier
        }
        .await
        .map_err(internal_status)?;
        self.state.queriers.write().await.insert(
            handle,
            QuerierEntry {
                owner: req.client_id,
                querier,
            },
        );
        Ok(Response::new(pb::HandleReply { handle }))
    }

    type GetStream = BoxReplyStream;

    async fn get(
        &self,
        request: Request<pb::QuerierGetRequest>,
    ) -> Result<Response<Self::GetStream>, Status> {
        let req = request.into_inner();
        let queriers = self.state.queriers.read().await;
        let entry = queriers
            .get(&req.handle)
            .ok_or_else(|| Status::not_found("querier handle not found"))?;
        ensure_owner(&req.client_id, &entry.owner)?;
        let mut builder = entry.querier.get();
        builder = builder.parameters(req.parameters);
        if !req.payload.is_empty() {
            builder = builder.payload(req.payload).encoding(Encoding::from(req.encoding));
        }
        if let Some(attachment) = opt_attachment(req.attachment) {
            builder = builder.attachment(attachment);
        }
        let replies = builder.await.map_err(internal_status)?;
        let stream = try_stream! {
            while let Ok(reply) = replies.recv_async().await {
                yield encode_reply(reply);
            }
        };
        Ok(Response::new(Box::pin(stream)))
    }

    async fn undeclare(
        &self,
        request: Request<pb::UndeclareRequest>,
    ) -> Result<Response<pb::Empty>, Status> {
        let req = request.into_inner();
        let entry = self
            .state
            .queriers
            .write()
            .await
            .remove(&req.handle)
            .ok_or_else(|| Status::not_found("querier handle not found"))?;
        ensure_owner(&req.client_id, &entry.owner)?;
        entry.querier.undeclare().await.map_err(internal_status)?;
        Ok(Response::new(pb::Empty {}))
    }
}

fn internal_status<E: std::fmt::Display>(err: E) -> Status {
    Status::internal(err.to_string())
}

async fn run(runtime: DynamicRuntime, config: Config, stop: Arc<Notify>) {
    let session = zenoh::session::init(runtime)
        .await
        .expect("failed to initialize shared zenoh session");
    let state = Arc::new(PluginState::new(session, config.clone()));
    let service = GrpcServer::new(state.clone());
    let listeners = match config.listeners() {
        Ok(listeners) => listeners,
        Err(err) => {
            warn!("invalid grpc listener configuration: {err}");
            return;
        }
    };

    let mut tasks = Vec::new();
    for listener in listeners {
        let stop = stop.clone();
        let service = service.clone();
        let cfg = state.config.clone();
        tasks.push(spawn_runtime(async move {
            match listener {
                ListenEndpoint::Tcp(addr) => {
                    info!("Starting zenoh gRPC TCP server on {addr}");
                    let listener = TcpListener::bind(&addr).await.expect("bind tcp");
                    let incoming = tokio_stream::wrappers::TcpListenerStream::new(listener);
                    let _ = Server::builder()
                        .max_frame_size(Some(cfg.max_recv_message_size as u32))
                        .max_concurrent_streams(None)
                        .add_service(SessionServiceServer::new(service.clone()))
                        .add_service(PublisherServiceServer::new(service.clone()))
                        .add_service(SubscriberServiceServer::new(service.clone()))
                        .add_service(QueryableServiceServer::new(service.clone()))
                        .add_service(QuerierServiceServer::new(service.clone()))
                        .serve_with_incoming_shutdown(incoming, async move {
                            stop.notified().await;
                        })
                        .await;
                }
                ListenEndpoint::Unix(path) => {
                    let _ = std::fs::remove_file(&path);
                    info!("Starting zenoh gRPC UDS server on {path}");
                    let listener = UnixListener::bind(&path).expect("bind uds");
                    let incoming = tokio_stream::wrappers::UnixListenerStream::new(listener);
                    let _ = Server::builder()
                        .max_frame_size(Some(cfg.max_recv_message_size as u32))
                        .add_service(SessionServiceServer::new(service.clone()))
                        .add_service(PublisherServiceServer::new(service.clone()))
                        .add_service(SubscriberServiceServer::new(service.clone()))
                        .add_service(QueryableServiceServer::new(service.clone()))
                        .add_service(QuerierServiceServer::new(service.clone()))
                        .serve_with_incoming_shutdown(incoming, async move {
                            stop.notified().await;
                        })
                        .await;
                }
            }
        }));
    }

    stop.notified().await;
    tokio::time::sleep(config.shutdown_grace_period).await;
    for task in tasks {
        task.abort();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener as StdTcpListener;

    use tokio::time::{sleep, timeout};
    use zenoh::{
        config::Config as ZenohConfig,
        internal::runtime::{DynamicRuntime, Runtime, RuntimeBuilder},
    };
    use zenoh_grpc_client_rs::{
        ConnectAddr, DeclarePublisherArgs, DeclareQuerierArgs, DeclareQueryableArgs,
        DeclareSubscriberArgs, GrpcSession, PublisherPutArgs, QuerierGetArgs, QueryReplyArgs,
        SessionGetArgs,
    };

    struct TestHarness {
        addr: String,
        stop: Arc<Notify>,
        _runtime: Runtime,
        _task: JoinHandle<()>,
    }

    impl Drop for TestHarness {
        fn drop(&mut self) {
            self.stop.notify_waiters();
        }
    }

    fn free_port() -> u16 {
        StdTcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port()
    }

    async fn start_harness() -> TestHarness {
        let port = free_port();
        let addr = format!("127.0.0.1:{port}");
        let mut runtime = RuntimeBuilder::new(ZenohConfig::default())
            .build()
            .await
            .unwrap();
        runtime.start().await.unwrap();
        let stop = Arc::new(Notify::new());
        let config = Config {
            host: "127.0.0.1".into(),
            port,
            ..Config::default()
        };
        let dynamic_runtime: DynamicRuntime = runtime.clone().into();
        let task = spawn_runtime(run(dynamic_runtime, config, stop.clone()));

        for _ in 0..20 {
            if GrpcSession::connect(ConnectAddr::Tcp(addr.clone()))
                .await
                .is_ok()
            {
                break;
            }
            sleep(Duration::from_millis(50)).await;
        }

        TestHarness {
            addr,
            stop,
            _runtime: runtime,
            _task: task,
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn pub_sub_e2e() {
        let harness = start_harness().await;
        let sub_client = GrpcSession::connect(ConnectAddr::Tcp(harness.addr.clone()))
            .await
            .unwrap();
        let pub_client = GrpcSession::connect(ConnectAddr::Tcp(harness.addr.clone()))
            .await
            .unwrap();

        let subscriber = sub_client
            .declare_subscriber(DeclareSubscriberArgs {
                key_expr: "demo/e2e/**".into(),
                ..Default::default()
            })
            .await
            .unwrap();
        let publisher = pub_client
            .declare_publisher(DeclarePublisherArgs {
                key_expr: "demo/e2e/value".into(),
                ..Default::default()
            })
            .await
            .unwrap();

        publisher
            .put(PublisherPutArgs {
                payload: b"hello".to_vec(),
                ..Default::default()
            })
            .await
            .unwrap();

        let event = timeout(Duration::from_secs(5), subscriber.receiver().recv_async())
            .await
            .unwrap()
            .unwrap();
        let sample = event.sample.unwrap();
        assert_eq!(sample.key_expr, "demo/e2e/value");
        assert_eq!(sample.payload, b"hello".to_vec());

        publisher.undeclare().await.unwrap();
        subscriber.undeclare().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn queryable_session_get_e2e() {
        let harness = start_harness().await;
        let queryable_client = GrpcSession::connect(ConnectAddr::Tcp(harness.addr.clone()))
            .await
            .unwrap();
        let getter_client = GrpcSession::connect(ConnectAddr::Tcp(harness.addr.clone()))
            .await
            .unwrap();

        let queryable = queryable_client
            .declare_queryable(DeclareQueryableArgs {
                key_expr: "demo/query/**".into(),
                ..Default::default()
            })
            .await
            .unwrap();

        let queryable_task = {
            let queryable = queryable;
            tokio::spawn(async move {
                let event = timeout(Duration::from_secs(5), queryable.receiver().recv_async())
                    .await
                    .unwrap()
                    .unwrap();
                let query = event.query.unwrap();
                queryable
                    .reply(QueryReplyArgs {
                        query_id: query.query_id,
                        key_expr: "demo/query/value".into(),
                        payload: b"world".to_vec(),
                        ..Default::default()
                    })
                    .await
                    .unwrap();
                queryable.undeclare().await.unwrap();
            })
        };

        let replies = getter_client
            .get(SessionGetArgs {
                selector: "demo/query/value".into(),
                ..Default::default()
            })
            .await
            .unwrap();
        let reply = timeout(Duration::from_secs(5), replies.recv_async())
            .await
            .unwrap()
            .unwrap();
        let sample = match reply.result.unwrap() {
            pb::reply::Result::Sample(sample) => sample,
            pb::reply::Result::Error(err) => panic!("unexpected error reply: {:?}", err),
        };
        assert_eq!(sample.key_expr, "demo/query/value");
        assert_eq!(sample.payload, b"world".to_vec());

        queryable_task.await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn queryable_querier_e2e() {
        let harness = start_harness().await;
        let queryable_client = GrpcSession::connect(ConnectAddr::Tcp(harness.addr.clone()))
            .await
            .unwrap();
        let querier_client = GrpcSession::connect(ConnectAddr::Tcp(harness.addr.clone()))
            .await
            .unwrap();

        let queryable = queryable_client
            .declare_queryable(DeclareQueryableArgs {
                key_expr: "demo/querier/**".into(),
                ..Default::default()
            })
            .await
            .unwrap();
        let querier = querier_client
            .declare_querier(DeclareQuerierArgs {
                key_expr: "demo/querier/**".into(),
                ..Default::default()
            })
            .await
            .unwrap();

        let queryable_task = {
            let queryable = queryable;
            tokio::spawn(async move {
                let event = timeout(Duration::from_secs(5), queryable.receiver().recv_async())
                    .await
                    .unwrap()
                    .unwrap();
                let query = event.query.unwrap();
                queryable
                    .reply(QueryReplyArgs {
                        query_id: query.query_id,
                        key_expr: "demo/querier/value".into(),
                        payload: b"querier".to_vec(),
                        ..Default::default()
                    })
                    .await
                    .unwrap();
                queryable.undeclare().await.unwrap();
            })
        };

        let replies = querier
            .get(QuerierGetArgs {
                parameters: String::new(),
                ..Default::default()
            })
            .await
            .unwrap();
        let reply = timeout(Duration::from_secs(5), replies.recv_async())
            .await
            .unwrap()
            .unwrap();
        let sample = match reply.result.unwrap() {
            pb::reply::Result::Sample(sample) => sample,
            pb::reply::Result::Error(err) => panic!("unexpected error reply: {:?}", err),
        };
        assert_eq!(sample.key_expr, "demo/querier/value");
        assert_eq!(sample.payload, b"querier".to_vec());

        querier.undeclare().await.unwrap();
        queryable_task.await.unwrap();
    }
}
