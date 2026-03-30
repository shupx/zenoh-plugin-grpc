use std::{
    collections::HashMap,
    ffi::{CStr, CString},
    mem,
    os::raw::{c_char, c_int, c_void},
    path::PathBuf,
    ptr,
    sync::{
        atomic::{AtomicPtr, Ordering},
        Arc, Mutex,
    },
};

use tokio::runtime::{Builder, Runtime};
use zenoh_grpc_client_rs::{
    ConnectAddr, DeclarePublisherArgs, DeclareQuerierArgs, DeclareQueryableArgs,
    DeclareSubscriberArgs, GrpcPublisher, GrpcQuerier, GrpcQuery, GrpcQueryable, GrpcSession,
    GrpcSubscriber, PublisherDeleteArgs, PublisherPutArgs, QuerierGetArgs, QueryableCallback,
    ReplyStream, SessionDeleteArgs, SessionGetArgs, SessionPutArgs, SubscriberCallback,
};
use zenoh_grpc_proto::v1 as pb;

const DEFAULT_ENDPOINT: &str = "unix:///tmp/zenoh-grpc.sock";

type SubscriberCallbackFn =
    Option<unsafe extern "C" fn(*const zgrpc_subscriber_event_t, *mut c_void)>;
type QueryableCallbackFn =
    Option<unsafe extern "C" fn(*mut zgrpc_queryable_t, *const zgrpc_query_ex_t, *mut c_void)>;
type DropCallbackFn = Option<unsafe extern "C" fn(*mut c_void)>;

fn runtime() -> Arc<Runtime> {
    static RT: std::sync::OnceLock<Arc<Runtime>> = std::sync::OnceLock::new();
    RT.get_or_init(|| {
        Arc::new(
            Builder::new_multi_thread()
                .worker_threads(2)
                .max_blocking_threads(16)
                .enable_all()
                .build()
                .expect("tokio runtime"),
        )
    })
    .clone()
}

struct SubscriberCallbackState {
    callback: SubscriberCallbackFn,
    on_drop: DropCallbackFn,
    context: AtomicPtr<c_void>,
}

impl Drop for SubscriberCallbackState {
    fn drop(&mut self) {
        if let Some(on_drop) = self.on_drop {
            let context = self.context.load(Ordering::Acquire);
            unsafe { on_drop(context) };
        }
    }
}

struct QueryableCallbackState {
    callback: QueryableCallbackFn,
    on_drop: DropCallbackFn,
    context: AtomicPtr<c_void>,
    self_ptr: AtomicPtr<zgrpc_queryable_t>,
}

impl Drop for QueryableCallbackState {
    fn drop(&mut self) {
        if let Some(on_drop) = self.on_drop {
            let context = self.context.load(Ordering::Acquire);
            unsafe { on_drop(context) };
        }
    }
}

#[repr(C)]
pub struct zgrpc_session_t {
    rt: Arc<Runtime>,
    inner: GrpcSession,
}

#[repr(C)]
pub struct zgrpc_publisher_t {
    rt: Arc<Runtime>,
    inner: GrpcPublisher,
}

#[repr(C)]
pub struct zgrpc_subscriber_t {
    rt: Arc<Runtime>,
    inner: GrpcSubscriber,
    _callback_state: Option<Arc<SubscriberCallbackState>>,
}

#[repr(C)]
pub struct zgrpc_queryable_t {
    rt: Arc<Runtime>,
    inner: GrpcQueryable,
    callback_state: Option<Arc<QueryableCallbackState>>,
    active_queries: Mutex<HashMap<u64, GrpcQuery>>,
}

#[repr(C)]
pub struct zgrpc_querier_t {
    rt: Arc<Runtime>,
    inner: GrpcQuerier,
}

#[repr(C)]
pub struct zgrpc_reply_stream_t {
    inner: ReplyStream,
}

#[repr(C)]
pub struct zgrpc_sample_t {
    pub key_expr: *mut c_char,
    pub payload: *mut u8,
    pub payload_len: usize,
}

#[repr(C)]
pub struct zgrpc_query_t {
    pub query_id: u64,
    pub key_expr: *mut c_char,
    pub parameters: *mut c_char,
    pub payload: *mut u8,
    pub payload_len: usize,
}

#[repr(C)]
pub struct zgrpc_reply_t {
    pub is_error: c_int,
    pub key_expr: *mut c_char,
    pub payload: *mut u8,
    pub payload_len: usize,
}

#[repr(C)]
pub struct zgrpc_source_info_t {
    pub is_valid: c_int,
    pub id: *mut c_char,
    pub sequence: u64,
}

#[repr(C)]
pub struct zgrpc_sample_ex_t {
    pub key_expr: *mut c_char,
    pub payload: *mut u8,
    pub payload_len: usize,
    pub encoding: *mut c_char,
    pub kind: c_int,
    pub attachment: *mut u8,
    pub attachment_len: usize,
    pub timestamp: *mut c_char,
    pub source_info: zgrpc_source_info_t,
}

#[repr(C)]
pub struct zgrpc_subscriber_event_t {
    pub has_sample: c_int,
    pub sample: zgrpc_sample_ex_t,
}

#[repr(C)]
pub struct zgrpc_query_ex_t {
    pub query_id: u64,
    pub selector: *mut c_char,
    pub key_expr: *mut c_char,
    pub parameters: *mut c_char,
    pub payload: *mut u8,
    pub payload_len: usize,
    pub encoding: *mut c_char,
    pub attachment: *mut u8,
    pub attachment_len: usize,
}

#[repr(C)]
pub struct zgrpc_reply_error_t {
    pub payload: *mut u8,
    pub payload_len: usize,
    pub encoding: *mut c_char,
}

#[repr(C)]
pub struct zgrpc_reply_ex_t {
    pub has_sample: c_int,
    pub sample: zgrpc_sample_ex_t,
    pub has_error: c_int,
    pub error: zgrpc_reply_error_t,
}

#[repr(C)]
pub struct zgrpc_session_put_options_t {
    pub encoding: *const c_char,
    pub congestion_control: c_int,
    pub priority: c_int,
    pub express: bool,
    pub attachment: *const u8,
    pub attachment_len: usize,
    pub timestamp: *const c_char,
    pub allowed_destination: c_int,
}

#[repr(C)]
pub struct zgrpc_session_delete_options_t {
    pub congestion_control: c_int,
    pub priority: c_int,
    pub express: bool,
    pub attachment: *const u8,
    pub attachment_len: usize,
    pub timestamp: *const c_char,
    pub allowed_destination: c_int,
}

#[repr(C)]
pub struct zgrpc_session_get_options_t {
    pub target: c_int,
    pub consolidation: c_int,
    pub timeout_ms: u64,
    pub payload: *const u8,
    pub payload_len: usize,
    pub encoding: *const c_char,
    pub attachment: *const u8,
    pub attachment_len: usize,
    pub allowed_destination: c_int,
}

#[repr(C)]
pub struct zgrpc_publisher_options_t {
    pub encoding: *const c_char,
    pub congestion_control: c_int,
    pub priority: c_int,
    pub express: bool,
    pub reliability: c_int,
    pub allowed_destination: c_int,
}

#[repr(C)]
pub struct zgrpc_subscriber_options_t {
    pub allowed_origin: c_int,
}

#[repr(C)]
pub struct zgrpc_queryable_options_t {
    pub complete: bool,
    pub allowed_origin: c_int,
}

#[repr(C)]
pub struct zgrpc_querier_options_t {
    pub target: c_int,
    pub consolidation: c_int,
    pub timeout_ms: u64,
    pub allowed_destination: c_int,
}

#[repr(C)]
pub struct zgrpc_querier_get_options_t {
    pub payload: *const u8,
    pub payload_len: usize,
    pub encoding: *const c_char,
    pub attachment: *const u8,
    pub attachment_len: usize,
}

#[repr(C)]
pub struct zgrpc_publisher_put_options_t {
    pub encoding: *const c_char,
    pub attachment: *const u8,
    pub attachment_len: usize,
    pub timestamp: *const c_char,
}

#[repr(C)]
pub struct zgrpc_publisher_delete_options_t {
    pub attachment: *const u8,
    pub attachment_len: usize,
    pub timestamp: *const c_char,
}

#[repr(C)]
pub struct zgrpc_query_reply_options_t {
    pub encoding: *const c_char,
    pub attachment: *const u8,
    pub attachment_len: usize,
    pub timestamp: *const c_char,
}

#[repr(C)]
pub struct zgrpc_query_reply_err_options_t {
    pub encoding: *const c_char,
}

#[repr(C)]
pub struct zgrpc_query_reply_delete_options_t {
    pub attachment: *const u8,
    pub attachment_len: usize,
    pub timestamp: *const c_char,
}

unsafe fn cstr(ptr: *const c_char) -> String {
    if ptr.is_null() {
        String::new()
    } else {
        CStr::from_ptr(ptr).to_string_lossy().into_owned()
    }
}

unsafe fn payload_vec(ptr: *const u8, len: usize) -> Vec<u8> {
    if ptr.is_null() || len == 0 {
        Vec::new()
    } else {
        std::slice::from_raw_parts(ptr, len).to_vec()
    }
}

fn into_c_string(value: impl Into<String>) -> *mut c_char {
    let value = value.into();
    if value.is_empty() {
        ptr::null_mut()
    } else {
        CString::new(value).unwrap().into_raw()
    }
}

fn into_c_bytes(mut bytes: Vec<u8>) -> (*mut u8, usize) {
    if bytes.is_empty() {
        (ptr::null_mut(), 0)
    } else {
        let len = bytes.len();
        let ptr = bytes.as_mut_ptr();
        mem::forget(bytes);
        (ptr, len)
    }
}

fn parse_endpoint(endpoint: Option<&str>) -> Option<ConnectAddr> {
    let endpoint = endpoint.unwrap_or(DEFAULT_ENDPOINT);
    if let Some(addr) = endpoint.strip_prefix("tcp://") {
        return Some(ConnectAddr::Tcp(addr.to_string()));
    }
    if let Some(path) = endpoint.strip_prefix("unix://") {
        return Some(ConnectAddr::Unix(PathBuf::from(path)));
    }
    None
}

unsafe fn take_session<'a>(session: *mut zgrpc_session_t) -> Option<&'a mut zgrpc_session_t> {
    session.as_mut()
}

unsafe fn take_publisher<'a>(
    publisher: *mut zgrpc_publisher_t,
) -> Option<&'a mut zgrpc_publisher_t> {
    publisher.as_mut()
}

unsafe fn take_subscriber<'a>(
    subscriber: *mut zgrpc_subscriber_t,
) -> Option<&'a mut zgrpc_subscriber_t> {
    subscriber.as_mut()
}

unsafe fn take_queryable<'a>(
    queryable: *mut zgrpc_queryable_t,
) -> Option<&'a mut zgrpc_queryable_t> {
    queryable.as_mut()
}

unsafe fn take_querier<'a>(querier: *mut zgrpc_querier_t) -> Option<&'a mut zgrpc_querier_t> {
    querier.as_mut()
}

unsafe fn take_reply_stream<'a>(
    stream: *mut zgrpc_reply_stream_t,
) -> Option<&'a mut zgrpc_reply_stream_t> {
    stream.as_mut()
}

fn status(result: Result<(), impl std::fmt::Display>) -> c_int {
    match result {
        Ok(()) => 0,
        Err(_) => -1,
    }
}

fn clear_source_info(info: &mut zgrpc_source_info_t) {
    info.is_valid = 0;
    info.id = ptr::null_mut();
    info.sequence = 0;
}

fn clear_sample(sample: &mut zgrpc_sample_t) {
    sample.key_expr = ptr::null_mut();
    sample.payload = ptr::null_mut();
    sample.payload_len = 0;
}

fn clear_query(query: &mut zgrpc_query_t) {
    query.query_id = 0;
    query.key_expr = ptr::null_mut();
    query.parameters = ptr::null_mut();
    query.payload = ptr::null_mut();
    query.payload_len = 0;
}

fn clear_reply(reply: &mut zgrpc_reply_t) {
    reply.is_error = 0;
    reply.key_expr = ptr::null_mut();
    reply.payload = ptr::null_mut();
    reply.payload_len = 0;
}

fn clear_sample_ex(sample: &mut zgrpc_sample_ex_t) {
    sample.key_expr = ptr::null_mut();
    sample.payload = ptr::null_mut();
    sample.payload_len = 0;
    sample.encoding = ptr::null_mut();
    sample.kind = 0;
    sample.attachment = ptr::null_mut();
    sample.attachment_len = 0;
    sample.timestamp = ptr::null_mut();
    clear_source_info(&mut sample.source_info);
}

fn clear_subscriber_event(event: &mut zgrpc_subscriber_event_t) {
    event.has_sample = 0;
    clear_sample_ex(&mut event.sample);
}

fn clear_query_ex(query: &mut zgrpc_query_ex_t) {
    query.query_id = 0;
    query.selector = ptr::null_mut();
    query.key_expr = ptr::null_mut();
    query.parameters = ptr::null_mut();
    query.payload = ptr::null_mut();
    query.payload_len = 0;
    query.encoding = ptr::null_mut();
    query.attachment = ptr::null_mut();
    query.attachment_len = 0;
}

fn clear_reply_error(error: &mut zgrpc_reply_error_t) {
    error.payload = ptr::null_mut();
    error.payload_len = 0;
    error.encoding = ptr::null_mut();
}

fn clear_reply_ex(reply: &mut zgrpc_reply_ex_t) {
    reply.has_sample = 0;
    clear_sample_ex(&mut reply.sample);
    reply.has_error = 0;
    clear_reply_error(&mut reply.error);
}

fn fill_source_info(dst: &mut zgrpc_source_info_t, source: &Option<pb::SourceInfo>) {
    clear_source_info(dst);
    if let Some(source) = source {
        dst.is_valid = 1;
        dst.id = into_c_string(source.id.clone());
        dst.sequence = source.sequence;
    }
}

fn fill_sample(dst: &mut zgrpc_sample_t, sample: &pb::Sample) {
    clear_sample(dst);
    let (payload, payload_len) = into_c_bytes(sample.payload.clone());
    dst.key_expr = into_c_string(sample.key_expr.clone());
    dst.payload = payload;
    dst.payload_len = payload_len;
}

fn fill_sample_ex(dst: &mut zgrpc_sample_ex_t, sample: &pb::Sample) {
    clear_sample_ex(dst);
    let (payload, payload_len) = into_c_bytes(sample.payload.clone());
    let (attachment, attachment_len) = into_c_bytes(sample.attachment.clone());
    dst.key_expr = into_c_string(sample.key_expr.clone());
    dst.payload = payload;
    dst.payload_len = payload_len;
    dst.encoding = into_c_string(sample.encoding.clone());
    dst.kind = sample.kind;
    dst.attachment = attachment;
    dst.attachment_len = attachment_len;
    dst.timestamp = into_c_string(sample.timestamp.clone());
    fill_source_info(&mut dst.source_info, &sample.source_info);
}

fn fill_subscriber_event(dst: &mut zgrpc_subscriber_event_t, event: &pb::SubscriberEvent) -> c_int {
    clear_subscriber_event(dst);
    if let Some(sample) = &event.sample {
        dst.has_sample = 1;
        fill_sample_ex(&mut dst.sample, sample);
        0
    } else {
        -1
    }
}

fn query_to_owned(query: &GrpcQuery) -> zgrpc_query_ex_t {
    let mut out = zgrpc_query_ex_t {
        query_id: query.query_id(),
        selector: ptr::null_mut(),
        key_expr: ptr::null_mut(),
        parameters: ptr::null_mut(),
        payload: ptr::null_mut(),
        payload_len: 0,
        encoding: ptr::null_mut(),
        attachment: ptr::null_mut(),
        attachment_len: 0,
    };
    out.selector = into_c_string(query.selector().to_string());
    out.key_expr = into_c_string(query.key_expr().to_string());
    out.parameters = into_c_string(query.parameters().to_string());
    let (payload, payload_len) = into_c_bytes(query.payload().to_vec());
    out.payload = payload;
    out.payload_len = payload_len;
    out.encoding = into_c_string(query.encoding().to_string());
    let (attachment, attachment_len) = into_c_bytes(query.attachment().to_vec());
    out.attachment = attachment;
    out.attachment_len = attachment_len;
    out
}

fn fill_legacy_query(dst: &mut zgrpc_query_t, query: &GrpcQuery) {
    clear_query(dst);
    dst.query_id = query.query_id();
    dst.key_expr = into_c_string(query.key_expr().to_string());
    dst.parameters = into_c_string(query.parameters().to_string());
    let (payload, payload_len) = into_c_bytes(query.payload().to_vec());
    dst.payload = payload;
    dst.payload_len = payload_len;
}

fn fill_reply_ex(dst: &mut zgrpc_reply_ex_t, reply: &pb::Reply) -> c_int {
    clear_reply_ex(dst);
    match &reply.result {
        Some(pb::reply::Result::Sample(sample)) => {
            dst.has_sample = 1;
            fill_sample_ex(&mut dst.sample, sample);
            0
        }
        Some(pb::reply::Result::Error(error)) => {
            dst.has_error = 1;
            let (payload, payload_len) = into_c_bytes(error.payload.clone());
            dst.error.payload = payload;
            dst.error.payload_len = payload_len;
            dst.error.encoding = into_c_string(error.encoding.clone());
            0
        }
        None => -1,
    }
}

unsafe fn session_put_args(
    key_expr: *const c_char,
    payload: *const u8,
    payload_len: usize,
    options: *const zgrpc_session_put_options_t,
) -> SessionPutArgs {
    let options = options.as_ref();
    SessionPutArgs {
        key_expr: cstr(key_expr),
        payload: payload_vec(payload, payload_len),
        encoding: options.map_or_else(String::new, |o| cstr(o.encoding)),
        congestion_control: options.map_or(0, |o| o.congestion_control),
        priority: options.map_or(0, |o| o.priority),
        express: options.map_or(false, |o| o.express),
        attachment: options.map_or_else(Vec::new, |o| payload_vec(o.attachment, o.attachment_len)),
        timestamp: options.map_or_else(String::new, |o| cstr(o.timestamp)),
        allowed_destination: options.map_or(0, |o| o.allowed_destination),
    }
}

unsafe fn session_delete_args(
    key_expr: *const c_char,
    options: *const zgrpc_session_delete_options_t,
) -> SessionDeleteArgs {
    let options = options.as_ref();
    SessionDeleteArgs {
        key_expr: cstr(key_expr),
        congestion_control: options.map_or(0, |o| o.congestion_control),
        priority: options.map_or(0, |o| o.priority),
        express: options.map_or(false, |o| o.express),
        attachment: options.map_or_else(Vec::new, |o| payload_vec(o.attachment, o.attachment_len)),
        timestamp: options.map_or_else(String::new, |o| cstr(o.timestamp)),
        allowed_destination: options.map_or(0, |o| o.allowed_destination),
    }
}

unsafe fn session_get_args(
    selector: *const c_char,
    options: *const zgrpc_session_get_options_t,
) -> SessionGetArgs {
    let options = options.as_ref();
    SessionGetArgs {
        selector: cstr(selector),
        target: options.map_or(0, |o| o.target),
        consolidation: options.map_or(0, |o| o.consolidation),
        timeout_ms: options.map_or(0, |o| o.timeout_ms),
        payload: options.map_or_else(Vec::new, |o| payload_vec(o.payload, o.payload_len)),
        encoding: options.map_or_else(String::new, |o| cstr(o.encoding)),
        attachment: options.map_or_else(Vec::new, |o| payload_vec(o.attachment, o.attachment_len)),
        allowed_destination: options.map_or(0, |o| o.allowed_destination),
    }
}

unsafe fn publisher_args(
    key_expr: *const c_char,
    options: *const zgrpc_publisher_options_t,
) -> DeclarePublisherArgs {
    let options = options.as_ref();
    DeclarePublisherArgs {
        key_expr: cstr(key_expr),
        encoding: options.map_or_else(String::new, |o| cstr(o.encoding)),
        congestion_control: options.map_or(0, |o| o.congestion_control),
        priority: options.map_or(0, |o| o.priority),
        express: options.map_or(false, |o| o.express),
        reliability: options.map_or(0, |o| o.reliability),
        allowed_destination: options.map_or(0, |o| o.allowed_destination),
    }
}

unsafe fn subscriber_args(
    key_expr: *const c_char,
    options: *const zgrpc_subscriber_options_t,
) -> DeclareSubscriberArgs {
    let options = options.as_ref();
    DeclareSubscriberArgs {
        key_expr: cstr(key_expr),
        allowed_origin: options.map_or(0, |o| o.allowed_origin),
    }
}

unsafe fn queryable_args(
    key_expr: *const c_char,
    options: *const zgrpc_queryable_options_t,
) -> DeclareQueryableArgs {
    let options = options.as_ref();
    DeclareQueryableArgs {
        key_expr: cstr(key_expr),
        complete: options.map_or(false, |o| o.complete),
        allowed_origin: options.map_or(0, |o| o.allowed_origin),
    }
}

unsafe fn querier_args(
    key_expr: *const c_char,
    options: *const zgrpc_querier_options_t,
) -> DeclareQuerierArgs {
    let options = options.as_ref();
    DeclareQuerierArgs {
        key_expr: cstr(key_expr),
        target: options.map_or(0, |o| o.target),
        consolidation: options.map_or(0, |o| o.consolidation),
        timeout_ms: options.map_or(0, |o| o.timeout_ms),
        allowed_destination: options.map_or(0, |o| o.allowed_destination),
    }
}

unsafe fn querier_get_args(
    parameters: *const c_char,
    options: *const zgrpc_querier_get_options_t,
) -> QuerierGetArgs {
    let options = options.as_ref();
    QuerierGetArgs {
        parameters: cstr(parameters),
        payload: options.map_or_else(Vec::new, |o| payload_vec(o.payload, o.payload_len)),
        encoding: options.map_or_else(String::new, |o| cstr(o.encoding)),
        attachment: options.map_or_else(Vec::new, |o| payload_vec(o.attachment, o.attachment_len)),
    }
}

unsafe fn publisher_put_args(
    payload: *const u8,
    payload_len: usize,
    options: *const zgrpc_publisher_put_options_t,
) -> PublisherPutArgs {
    let options = options.as_ref();
    PublisherPutArgs {
        payload: payload_vec(payload, payload_len),
        encoding: options.map_or_else(String::new, |o| cstr(o.encoding)),
        attachment: options.map_or_else(Vec::new, |o| payload_vec(o.attachment, o.attachment_len)),
        timestamp: options.map_or_else(String::new, |o| cstr(o.timestamp)),
    }
}

unsafe fn publisher_delete_args(
    options: *const zgrpc_publisher_delete_options_t,
) -> PublisherDeleteArgs {
    let options = options.as_ref();
    PublisherDeleteArgs {
        attachment: options.map_or_else(Vec::new, |o| payload_vec(o.attachment, o.attachment_len)),
        timestamp: options.map_or_else(String::new, |o| cstr(o.timestamp)),
    }
}

fn finish_all_active_queries(queryable: &mut zgrpc_queryable_t) {
    let queries = {
        let mut active = queryable.active_queries.lock().expect("active_queries poisoned");
        mem::take(&mut *active)
    };
    for (_query_id, query) in queries {
        let _ = queryable.rt.block_on(query.finish());
    }
}

fn recv_query_into_map(queryable: &mut zgrpc_queryable_t, query: GrpcQuery) {
    queryable
        .active_queries
        .lock()
        .expect("active_queries poisoned")
        .insert(query.query_id(), query);
}

fn remove_query(queryable: &mut zgrpc_queryable_t, query_id: u64) -> Option<GrpcQuery> {
    queryable
        .active_queries
        .lock()
        .expect("active_queries poisoned")
        .remove(&query_id)
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_session_connect(endpoint: *const c_char) -> *mut zgrpc_session_t {
    let rt = runtime();
    let endpoint = if endpoint.is_null() {
        None
    } else {
        Some(cstr(endpoint))
    };
    let Some(addr) = parse_endpoint(endpoint.as_deref()) else {
        return ptr::null_mut();
    };
    match rt.block_on(GrpcSession::connect(addr)) {
        Ok(inner) => Box::into_raw(Box::new(zgrpc_session_t { rt, inner })),
        Err(_) => ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_open_tcp(addr: *const c_char) -> *mut zgrpc_session_t {
    let rt = runtime();
    match rt.block_on(GrpcSession::connect(ConnectAddr::Tcp(cstr(addr)))) {
        Ok(inner) => Box::into_raw(Box::new(zgrpc_session_t { rt, inner })),
        Err(_) => ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_open_unix(path: *const c_char) -> *mut zgrpc_session_t {
    let rt = runtime();
    match rt.block_on(GrpcSession::connect(ConnectAddr::Unix(PathBuf::from(
        cstr(path),
    )))) {
        Ok(inner) => Box::into_raw(Box::new(zgrpc_session_t { rt, inner })),
        Err(_) => ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_session_info(session: *mut zgrpc_session_t) -> *mut c_char {
    let Some(session) = take_session(session) else {
        return ptr::null_mut();
    };
    match session.rt.block_on(session.inner.info()) {
        Ok(info) => into_c_string(info.zid),
        Err(_) => ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_close(session: *mut zgrpc_session_t) {
    if !session.is_null() {
        let session = Box::from_raw(session);
        let _ = session.rt.block_on(session.inner.cleanup());
    }
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_session_put_options_default(options: *mut zgrpc_session_put_options_t) {
    if let Some(options) = options.as_mut() {
        ptr::write_bytes(options, 0, 1);
    }
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_session_delete_options_default(
    options: *mut zgrpc_session_delete_options_t,
) {
    if let Some(options) = options.as_mut() {
        ptr::write_bytes(options, 0, 1);
    }
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_session_get_options_default(options: *mut zgrpc_session_get_options_t) {
    if let Some(options) = options.as_mut() {
        ptr::write_bytes(options, 0, 1);
    }
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_publisher_options_default(options: *mut zgrpc_publisher_options_t) {
    if let Some(options) = options.as_mut() {
        ptr::write_bytes(options, 0, 1);
    }
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_subscriber_options_default(options: *mut zgrpc_subscriber_options_t) {
    if let Some(options) = options.as_mut() {
        ptr::write_bytes(options, 0, 1);
    }
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_queryable_options_default(options: *mut zgrpc_queryable_options_t) {
    if let Some(options) = options.as_mut() {
        ptr::write_bytes(options, 0, 1);
    }
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_querier_options_default(options: *mut zgrpc_querier_options_t) {
    if let Some(options) = options.as_mut() {
        ptr::write_bytes(options, 0, 1);
    }
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_querier_get_options_default(
    options: *mut zgrpc_querier_get_options_t,
) {
    if let Some(options) = options.as_mut() {
        ptr::write_bytes(options, 0, 1);
    }
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_publisher_put_options_default(
    options: *mut zgrpc_publisher_put_options_t,
) {
    if let Some(options) = options.as_mut() {
        ptr::write_bytes(options, 0, 1);
    }
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_publisher_delete_options_default(
    options: *mut zgrpc_publisher_delete_options_t,
) {
    if let Some(options) = options.as_mut() {
        ptr::write_bytes(options, 0, 1);
    }
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_query_reply_options_default(
    options: *mut zgrpc_query_reply_options_t,
) {
    if let Some(options) = options.as_mut() {
        ptr::write_bytes(options, 0, 1);
    }
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_query_reply_err_options_default(
    options: *mut zgrpc_query_reply_err_options_t,
) {
    if let Some(options) = options.as_mut() {
        ptr::write_bytes(options, 0, 1);
    }
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_query_reply_delete_options_default(
    options: *mut zgrpc_query_reply_delete_options_t,
) {
    if let Some(options) = options.as_mut() {
        ptr::write_bytes(options, 0, 1);
    }
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_session_put(
    session: *mut zgrpc_session_t,
    key_expr: *const c_char,
    payload: *const u8,
    payload_len: usize,
) -> c_int {
    zgrpc_session_put_with_options(session, key_expr, payload, payload_len, ptr::null())
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_session_put_with_options(
    session: *mut zgrpc_session_t,
    key_expr: *const c_char,
    payload: *const u8,
    payload_len: usize,
    options: *const zgrpc_session_put_options_t,
) -> c_int {
    let Some(session) = take_session(session) else {
        return -1;
    };
    status(
        session
            .rt
            .block_on(session.inner.put(session_put_args(key_expr, payload, payload_len, options))),
    )
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_session_delete(
    session: *mut zgrpc_session_t,
    key_expr: *const c_char,
) -> c_int {
    zgrpc_session_delete_with_options(session, key_expr, ptr::null())
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_session_delete_with_options(
    session: *mut zgrpc_session_t,
    key_expr: *const c_char,
    options: *const zgrpc_session_delete_options_t,
) -> c_int {
    let Some(session) = take_session(session) else {
        return -1;
    };
    status(
        session
            .rt
            .block_on(session.inner.delete(session_delete_args(key_expr, options))),
    )
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_session_get(
    session: *mut zgrpc_session_t,
    selector: *const c_char,
    options: *const zgrpc_session_get_options_t,
) -> *mut zgrpc_reply_stream_t {
    let Some(session) = take_session(session) else {
        return ptr::null_mut();
    };
    match session
        .rt
        .block_on(session.inner.get(session_get_args(selector, options)))
    {
        Ok(inner) => Box::into_raw(Box::new(zgrpc_reply_stream_t { inner })),
        Err(_) => ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_declare_publisher(
    session: *mut zgrpc_session_t,
    key_expr: *const c_char,
) -> *mut zgrpc_publisher_t {
    zgrpc_declare_publisher_with_options(session, key_expr, ptr::null())
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_declare_publisher_with_options(
    session: *mut zgrpc_session_t,
    key_expr: *const c_char,
    options: *const zgrpc_publisher_options_t,
) -> *mut zgrpc_publisher_t {
    let Some(session) = take_session(session) else {
        return ptr::null_mut();
    };
    match session
        .rt
        .block_on(session.inner.declare_publisher(publisher_args(key_expr, options)))
    {
        Ok(inner) => Box::into_raw(Box::new(zgrpc_publisher_t {
            rt: session.rt.clone(),
            inner,
        })),
        Err(_) => ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_publisher_put(
    publisher: *mut zgrpc_publisher_t,
    payload: *const u8,
    payload_len: usize,
) -> c_int {
    zgrpc_publisher_put_with_options(publisher, payload, payload_len, ptr::null())
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_publisher_put_with_options(
    publisher: *mut zgrpc_publisher_t,
    payload: *const u8,
    payload_len: usize,
    options: *const zgrpc_publisher_put_options_t,
) -> c_int {
    let Some(publisher) = take_publisher(publisher) else {
        return -1;
    };
    status(
        publisher
            .rt
            .block_on(publisher.inner.put(publisher_put_args(payload, payload_len, options))),
    )
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_publisher_delete(publisher: *mut zgrpc_publisher_t) -> c_int {
    zgrpc_publisher_delete_with_options(publisher, ptr::null())
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_publisher_delete_with_options(
    publisher: *mut zgrpc_publisher_t,
    options: *const zgrpc_publisher_delete_options_t,
) -> c_int {
    let Some(publisher) = take_publisher(publisher) else {
        return -1;
    };
    status(
        publisher
            .rt
            .block_on(publisher.inner.delete(publisher_delete_args(options))),
    )
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_publisher_send_dropped_count(
    publisher: *mut zgrpc_publisher_t,
) -> u64 {
    take_publisher(publisher)
        .map(|publisher| publisher.inner.send_dropped_count())
        .unwrap_or(0)
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_publisher_undeclare(publisher: *mut zgrpc_publisher_t) -> c_int {
    if publisher.is_null() {
        return -1;
    }
    let publisher = Box::from_raw(publisher);
    status(publisher.rt.block_on(publisher.inner.undeclare()))
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_declare_subscriber(
    session: *mut zgrpc_session_t,
    key_expr: *const c_char,
) -> *mut zgrpc_subscriber_t {
    zgrpc_declare_subscriber_with_options(session, key_expr, ptr::null())
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_declare_subscriber_with_options(
    session: *mut zgrpc_session_t,
    key_expr: *const c_char,
    options: *const zgrpc_subscriber_options_t,
) -> *mut zgrpc_subscriber_t {
    let Some(session) = take_session(session) else {
        return ptr::null_mut();
    };
    match session
        .rt
        .block_on(session.inner.declare_subscriber(subscriber_args(key_expr, options), None))
    {
        Ok(inner) => Box::into_raw(Box::new(zgrpc_subscriber_t {
            rt: session.rt.clone(),
            inner,
            _callback_state: None,
        })),
        Err(_) => ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_declare_subscriber_with_callback(
    session: *mut zgrpc_session_t,
    key_expr: *const c_char,
    callback: SubscriberCallbackFn,
    context: *mut c_void,
    on_drop: DropCallbackFn,
    options: *const zgrpc_subscriber_options_t,
) -> *mut zgrpc_subscriber_t {
    let Some(session) = take_session(session) else {
        return ptr::null_mut();
    };
    let Some(callback_fn) = callback else {
        return ptr::null_mut();
    };
    let state = Arc::new(SubscriberCallbackState {
        callback: Some(callback_fn),
        on_drop,
        context: AtomicPtr::new(context),
    });
    let state_for_callback = state.clone();
    let callback: SubscriberCallback = Arc::new(move |event: pb::SubscriberEvent| {
        let mut event_out = zgrpc_subscriber_event_t {
            has_sample: 0,
            sample: zgrpc_sample_ex_t {
                key_expr: ptr::null_mut(),
                payload: ptr::null_mut(),
                payload_len: 0,
                encoding: ptr::null_mut(),
                kind: 0,
                attachment: ptr::null_mut(),
                attachment_len: 0,
                timestamp: ptr::null_mut(),
                source_info: zgrpc_source_info_t {
                    is_valid: 0,
                    id: ptr::null_mut(),
                    sequence: 0,
                },
            },
        };
        let _ = fill_subscriber_event(&mut event_out, &event);
        if let Some(callback) = state_for_callback.callback {
            unsafe {
                callback(
                    &event_out,
                    state_for_callback.context.load(Ordering::Acquire),
                );
            }
        }
        unsafe { zgrpc_subscriber_event_free(&mut event_out) };
    });
    match session.rt.block_on(session.inner.declare_subscriber(
        subscriber_args(key_expr, options),
        Some(callback),
    )) {
        Ok(inner) => Box::into_raw(Box::new(zgrpc_subscriber_t {
            rt: session.rt.clone(),
            inner,
            _callback_state: Some(state),
        })),
        Err(_) => ptr::null_mut(),
    }
}

fn subscriber_recv_event_impl(
    subscriber: &mut zgrpc_subscriber_t,
    event_out: *mut zgrpc_subscriber_event_t,
    non_blocking: bool,
) -> c_int {
    unsafe {
        let Some(event_out) = event_out.as_mut() else {
            return -1;
        };
        clear_subscriber_event(event_out);
        let event = if non_blocking {
            match subscriber.inner.try_recv() {
                Ok(Some(event)) => event,
                Ok(None) => return 1,
                Err(_) => return -1,
            }
        } else {
            match subscriber.inner.recv() {
                Ok(event) => event,
                Err(_) => return -1,
            }
        };
        fill_subscriber_event(event_out, &event)
    }
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_subscriber_recv(
    subscriber: *mut zgrpc_subscriber_t,
    sample_out: *mut zgrpc_sample_t,
) -> c_int {
    let Some(subscriber) = take_subscriber(subscriber) else {
        return -1;
    };
    let Some(sample_out) = sample_out.as_mut() else {
        return -1;
    };
    clear_sample(sample_out);
    let event = match subscriber.inner.recv() {
        Ok(event) => event,
        Err(_) => return -1,
    };
    match event.sample {
        Some(sample) => {
            fill_sample(sample_out, &sample);
            0
        }
        None => -1,
    }
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_subscriber_try_recv(
    subscriber: *mut zgrpc_subscriber_t,
    sample_out: *mut zgrpc_sample_t,
) -> c_int {
    let Some(subscriber) = take_subscriber(subscriber) else {
        return -1;
    };
    let Some(sample_out) = sample_out.as_mut() else {
        return -1;
    };
    clear_sample(sample_out);
    match subscriber.inner.try_recv() {
        Ok(Some(event)) => match event.sample {
            Some(sample) => {
                fill_sample(sample_out, &sample);
                0
            }
            None => -1,
        },
        Ok(None) => 1,
        Err(_) => -1,
    }
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_subscriber_recv_event(
    subscriber: *mut zgrpc_subscriber_t,
    event_out: *mut zgrpc_subscriber_event_t,
) -> c_int {
    let Some(subscriber) = take_subscriber(subscriber) else {
        return -1;
    };
    subscriber_recv_event_impl(subscriber, event_out, false)
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_subscriber_try_recv_event(
    subscriber: *mut zgrpc_subscriber_t,
    event_out: *mut zgrpc_subscriber_event_t,
) -> c_int {
    let Some(subscriber) = take_subscriber(subscriber) else {
        return -1;
    };
    subscriber_recv_event_impl(subscriber, event_out, true)
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_subscriber_dropped_count(
    subscriber: *mut zgrpc_subscriber_t,
) -> u64 {
    take_subscriber(subscriber)
        .map(|subscriber| subscriber.inner.dropped_count())
        .unwrap_or(0)
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_subscriber_undeclare(subscriber: *mut zgrpc_subscriber_t) -> c_int {
    if subscriber.is_null() {
        return -1;
    }
    let subscriber = Box::from_raw(subscriber);
    status(subscriber.rt.block_on(subscriber.inner.undeclare()))
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_declare_queryable(
    session: *mut zgrpc_session_t,
    key_expr: *const c_char,
) -> *mut zgrpc_queryable_t {
    zgrpc_declare_queryable_with_options(session, key_expr, ptr::null())
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_declare_queryable_with_options(
    session: *mut zgrpc_session_t,
    key_expr: *const c_char,
    options: *const zgrpc_queryable_options_t,
) -> *mut zgrpc_queryable_t {
    let Some(session) = take_session(session) else {
        return ptr::null_mut();
    };
    match session
        .rt
        .block_on(session.inner.declare_queryable(queryable_args(key_expr, options), None))
    {
        Ok(inner) => Box::into_raw(Box::new(zgrpc_queryable_t {
            rt: session.rt.clone(),
            inner,
            callback_state: None,
            active_queries: Mutex::new(HashMap::new()),
        })),
        Err(_) => ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_declare_queryable_with_callback(
    session: *mut zgrpc_session_t,
    key_expr: *const c_char,
    callback: QueryableCallbackFn,
    context: *mut c_void,
    on_drop: DropCallbackFn,
    options: *const zgrpc_queryable_options_t,
) -> *mut zgrpc_queryable_t {
    let Some(session) = take_session(session) else {
        return ptr::null_mut();
    };
    let Some(callback_fn) = callback else {
        return ptr::null_mut();
    };
    let state = Arc::new(QueryableCallbackState {
        callback: Some(callback_fn),
        on_drop,
        context: AtomicPtr::new(context),
        self_ptr: AtomicPtr::new(ptr::null_mut()),
    });
    let state_for_callback = state.clone();
    let callback: QueryableCallback = Arc::new(move |query: GrpcQuery| {
        let self_ptr = state_for_callback.self_ptr.load(Ordering::Acquire);
        if self_ptr.is_null() {
            return;
        }
        let query_id = query.query_id();
        let mut query_out = query_to_owned(&query);
        unsafe {
            (*self_ptr)
                .active_queries
                .lock()
                .expect("active_queries poisoned")
                .insert(query_id, query);
        }
        if let Some(callback) = state_for_callback.callback {
            unsafe {
                callback(
                    self_ptr,
                    &query_out,
                    state_for_callback.context.load(Ordering::Acquire),
                );
            }
        }
        unsafe { zgrpc_query_ex_free(&mut query_out) };
        let query = unsafe {
            (*self_ptr)
                .active_queries
                .lock()
                .expect("active_queries poisoned")
                .remove(&query_id)
        };
        if let Some(query) = query {
            let _ = unsafe { (*self_ptr).rt.block_on(query.finish()) };
        }
    });
    match session.rt.block_on(session.inner.declare_queryable(
        queryable_args(key_expr, options),
        Some(callback),
    )) {
        Ok(inner) => {
            let mut queryable = Box::new(zgrpc_queryable_t {
                rt: session.rt.clone(),
                inner,
                callback_state: Some(state.clone()),
                active_queries: Mutex::new(HashMap::new()),
            });
            let self_ptr = queryable.as_mut() as *mut zgrpc_queryable_t;
            state.self_ptr.store(self_ptr, Ordering::Release);
            Box::into_raw(queryable)
        }
        Err(_) => ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_queryable_recv(
    queryable: *mut zgrpc_queryable_t,
    query_out: *mut zgrpc_query_t,
) -> c_int {
    let Some(queryable) = take_queryable(queryable) else {
        return -1;
    };
    let Some(query_out) = query_out.as_mut() else {
        return -1;
    };
    clear_query(query_out);
    let query = match queryable.inner.recv() {
        Ok(query) => query,
        Err(_) => return -1,
    };
    fill_legacy_query(query_out, &query);
    recv_query_into_map(queryable, query);
    0
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_queryable_try_recv(
    queryable: *mut zgrpc_queryable_t,
    query_out: *mut zgrpc_query_t,
) -> c_int {
    let Some(queryable) = take_queryable(queryable) else {
        return -1;
    };
    let Some(query_out) = query_out.as_mut() else {
        return -1;
    };
    clear_query(query_out);
    let query = match queryable.inner.try_recv() {
        Ok(Some(query)) => query,
        Ok(None) => return 1,
        Err(_) => return -1,
    };
    fill_legacy_query(query_out, &query);
    recv_query_into_map(queryable, query);
    0
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_queryable_recv_ex(
    queryable: *mut zgrpc_queryable_t,
    query_out: *mut zgrpc_query_ex_t,
) -> c_int {
    let Some(queryable) = take_queryable(queryable) else {
        return -1;
    };
    let Some(query_out) = query_out.as_mut() else {
        return -1;
    };
    clear_query_ex(query_out);
    let query = match queryable.inner.recv() {
        Ok(query) => query,
        Err(_) => return -1,
    };
    *query_out = query_to_owned(&query);
    recv_query_into_map(queryable, query);
    0
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_queryable_try_recv_ex(
    queryable: *mut zgrpc_queryable_t,
    query_out: *mut zgrpc_query_ex_t,
) -> c_int {
    let Some(queryable) = take_queryable(queryable) else {
        return -1;
    };
    let Some(query_out) = query_out.as_mut() else {
        return -1;
    };
    clear_query_ex(query_out);
    let query = match queryable.inner.try_recv() {
        Ok(Some(query)) => query,
        Ok(None) => return 1,
        Err(_) => return -1,
    };
    *query_out = query_to_owned(&query);
    recv_query_into_map(queryable, query);
    0
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_queryable_reply(
    queryable: *mut zgrpc_queryable_t,
    query_id: u64,
    key_expr: *const c_char,
    payload: *const u8,
    payload_len: usize,
) -> c_int {
    zgrpc_queryable_reply_with_options(
        queryable,
        query_id,
        key_expr,
        payload,
        payload_len,
        ptr::null(),
    )
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_queryable_reply_with_options(
    queryable: *mut zgrpc_queryable_t,
    query_id: u64,
    key_expr: *const c_char,
    payload: *const u8,
    payload_len: usize,
    options: *const zgrpc_query_reply_options_t,
) -> c_int {
    let Some(queryable) = take_queryable(queryable) else {
        return -1;
    };
    let options = options.as_ref();
    let active = queryable.active_queries.lock().expect("active_queries poisoned");
    let Some(query) = active.get(&query_id) else {
        return -1;
    };
    status(queryable.rt.block_on(query.reply(
        cstr(key_expr),
        payload_vec(payload, payload_len),
        options.map_or_else(String::new, |o| cstr(o.encoding)),
        options.map_or_else(Vec::new, |o| payload_vec(o.attachment, o.attachment_len)),
        options.map_or_else(String::new, |o| cstr(o.timestamp)),
    )))
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_queryable_reply_err(
    queryable: *mut zgrpc_queryable_t,
    query_id: u64,
    payload: *const u8,
    payload_len: usize,
) -> c_int {
    zgrpc_queryable_reply_err_with_options(queryable, query_id, payload, payload_len, ptr::null())
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_queryable_reply_err_with_options(
    queryable: *mut zgrpc_queryable_t,
    query_id: u64,
    payload: *const u8,
    payload_len: usize,
    options: *const zgrpc_query_reply_err_options_t,
) -> c_int {
    let Some(queryable) = take_queryable(queryable) else {
        return -1;
    };
    let options = options.as_ref();
    let active = queryable.active_queries.lock().expect("active_queries poisoned");
    let Some(query) = active.get(&query_id) else {
        return -1;
    };
    status(queryable.rt.block_on(query.reply_err(
        payload_vec(payload, payload_len),
        options.map_or_else(String::new, |o| cstr(o.encoding)),
    )))
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_queryable_reply_delete(
    queryable: *mut zgrpc_queryable_t,
    query_id: u64,
    key_expr: *const c_char,
) -> c_int {
    zgrpc_queryable_reply_delete_with_options(queryable, query_id, key_expr, ptr::null())
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_queryable_reply_delete_with_options(
    queryable: *mut zgrpc_queryable_t,
    query_id: u64,
    key_expr: *const c_char,
    options: *const zgrpc_query_reply_delete_options_t,
) -> c_int {
    let Some(queryable) = take_queryable(queryable) else {
        return -1;
    };
    let options = options.as_ref();
    let active = queryable.active_queries.lock().expect("active_queries poisoned");
    let Some(query) = active.get(&query_id) else {
        return -1;
    };
    status(queryable.rt.block_on(query.reply_delete(
        cstr(key_expr),
        options.map_or_else(Vec::new, |o| payload_vec(o.attachment, o.attachment_len)),
        options.map_or_else(String::new, |o| cstr(o.timestamp)),
    )))
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_queryable_finish(
    queryable: *mut zgrpc_queryable_t,
    query_id: u64,
) -> c_int {
    let Some(queryable) = take_queryable(queryable) else {
        return -1;
    };
    let Some(query) = remove_query(queryable, query_id) else {
        return -1;
    };
    status(queryable.rt.block_on(query.finish()))
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_queryable_dropped_count(
    queryable: *mut zgrpc_queryable_t,
    count_out: *mut u64,
) -> c_int {
    let Some(queryable) = take_queryable(queryable) else {
        return -1;
    };
    let Some(count_out) = count_out.as_mut() else {
        return -1;
    };
    match queryable.inner.dropped_count() {
        Ok(count) => {
            *count_out = count;
            0
        }
        Err(_) => -1,
    }
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_queryable_is_closed(
    queryable: *mut zgrpc_queryable_t,
    is_closed_out: *mut c_int,
) -> c_int {
    let Some(queryable) = take_queryable(queryable) else {
        return -1;
    };
    let Some(is_closed_out) = is_closed_out.as_mut() else {
        return -1;
    };
    match queryable.inner.is_closed() {
        Ok(is_closed) => {
            *is_closed_out = if is_closed { 1 } else { 0 };
            0
        }
        Err(_) => -1,
    }
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_queryable_send_dropped_count(
    queryable: *mut zgrpc_queryable_t,
) -> u64 {
    take_queryable(queryable)
        .map(|queryable| queryable.inner.send_dropped_count())
        .unwrap_or(0)
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_queryable_undeclare(queryable: *mut zgrpc_queryable_t) -> c_int {
    if queryable.is_null() {
        return -1;
    }
    let mut queryable = Box::from_raw(queryable);
    finish_all_active_queries(&mut queryable);
    if let Some(state) = &queryable.callback_state {
        state.self_ptr.store(ptr::null_mut(), Ordering::Release);
    }
    status(queryable.rt.block_on(queryable.inner.undeclare()))
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_declare_querier(
    session: *mut zgrpc_session_t,
    key_expr: *const c_char,
) -> *mut zgrpc_querier_t {
    zgrpc_declare_querier_with_options(session, key_expr, ptr::null())
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_declare_querier_with_options(
    session: *mut zgrpc_session_t,
    key_expr: *const c_char,
    options: *const zgrpc_querier_options_t,
) -> *mut zgrpc_querier_t {
    let Some(session) = take_session(session) else {
        return ptr::null_mut();
    };
    match session
        .rt
        .block_on(session.inner.declare_querier(querier_args(key_expr, options)))
    {
        Ok(inner) => Box::into_raw(Box::new(zgrpc_querier_t {
            rt: session.rt.clone(),
            inner,
        })),
        Err(_) => ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_querier_get(
    querier: *mut zgrpc_querier_t,
    parameters: *const c_char,
    reply_out: *mut zgrpc_reply_t,
) -> c_int {
    zgrpc_querier_get_with_options(querier, parameters, ptr::null(), reply_out)
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_querier_get_with_options(
    querier: *mut zgrpc_querier_t,
    parameters: *const c_char,
    options: *const zgrpc_querier_get_options_t,
    reply_out: *mut zgrpc_reply_t,
) -> c_int {
    let stream = zgrpc_querier_get_stream(querier, parameters, options);
    if stream.is_null() {
        return -1;
    }
    let mut reply_ex = zgrpc_reply_ex_t {
        has_sample: 0,
        sample: zgrpc_sample_ex_t {
            key_expr: ptr::null_mut(),
            payload: ptr::null_mut(),
            payload_len: 0,
            encoding: ptr::null_mut(),
            kind: 0,
            attachment: ptr::null_mut(),
            attachment_len: 0,
            timestamp: ptr::null_mut(),
            source_info: zgrpc_source_info_t {
                is_valid: 0,
                id: ptr::null_mut(),
                sequence: 0,
            },
        },
        has_error: 0,
        error: zgrpc_reply_error_t {
            payload: ptr::null_mut(),
            payload_len: 0,
            encoding: ptr::null_mut(),
        },
    };
    let rc = zgrpc_reply_stream_recv(stream, &mut reply_ex);
    zgrpc_reply_stream_drop(stream);
    if rc != 0 {
        zgrpc_reply_ex_free(&mut reply_ex);
        return rc;
    }
    let Some(reply_out) = reply_out.as_mut() else {
        zgrpc_reply_ex_free(&mut reply_ex);
        return -1;
    };
    clear_reply(reply_out);
    if reply_ex.has_sample != 0 {
        reply_out.is_error = 0;
        reply_out.key_expr = reply_ex.sample.key_expr;
        reply_out.payload = reply_ex.sample.payload;
        reply_out.payload_len = reply_ex.sample.payload_len;
        reply_ex.sample.key_expr = ptr::null_mut();
        reply_ex.sample.payload = ptr::null_mut();
        reply_ex.sample.payload_len = 0;
    } else if reply_ex.has_error != 0 {
        reply_out.is_error = 1;
        reply_out.payload = reply_ex.error.payload;
        reply_out.payload_len = reply_ex.error.payload_len;
        reply_ex.error.payload = ptr::null_mut();
        reply_ex.error.payload_len = 0;
    } else {
        zgrpc_reply_ex_free(&mut reply_ex);
        return -1;
    }
    zgrpc_reply_ex_free(&mut reply_ex);
    0
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_querier_get_stream(
    querier: *mut zgrpc_querier_t,
    parameters: *const c_char,
    options: *const zgrpc_querier_get_options_t,
) -> *mut zgrpc_reply_stream_t {
    let Some(querier) = take_querier(querier) else {
        return ptr::null_mut();
    };
    match querier
        .rt
        .block_on(querier.inner.get(querier_get_args(parameters, options)))
    {
        Ok(inner) => Box::into_raw(Box::new(zgrpc_reply_stream_t { inner })),
        Err(_) => ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_querier_undeclare(querier: *mut zgrpc_querier_t) -> c_int {
    if querier.is_null() {
        return -1;
    }
    let querier = Box::from_raw(querier);
    status(querier.rt.block_on(querier.inner.undeclare()))
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_reply_stream_recv(
    stream: *mut zgrpc_reply_stream_t,
    reply_out: *mut zgrpc_reply_ex_t,
) -> c_int {
    let Some(stream) = take_reply_stream(stream) else {
        return -1;
    };
    let Some(reply_out) = reply_out.as_mut() else {
        return -1;
    };
    clear_reply_ex(reply_out);
    match stream.inner.recv() {
        Ok(reply) => fill_reply_ex(reply_out, &reply),
        Err(_) => -1,
    }
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_reply_stream_try_recv(
    stream: *mut zgrpc_reply_stream_t,
    reply_out: *mut zgrpc_reply_ex_t,
) -> c_int {
    let Some(stream) = take_reply_stream(stream) else {
        return -1;
    };
    let Some(reply_out) = reply_out.as_mut() else {
        return -1;
    };
    clear_reply_ex(reply_out);
    match stream.inner.try_recv() {
        Ok(Some(reply)) => fill_reply_ex(reply_out, &reply),
        Ok(None) => 1,
        Err(_) => {
            if stream.inner.is_closed() {
                1
            } else {
                -1
            }
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_reply_stream_dropped_count(
    stream: *mut zgrpc_reply_stream_t,
) -> u64 {
    take_reply_stream(stream)
        .map(|stream| stream.inner.dropped_count())
        .unwrap_or(0)
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_reply_stream_is_closed(
    stream: *mut zgrpc_reply_stream_t,
) -> c_int {
    take_reply_stream(stream)
        .map(|stream| if stream.inner.is_closed() { 1 } else { 0 })
        .unwrap_or(1)
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_reply_stream_drop(stream: *mut zgrpc_reply_stream_t) {
    if !stream.is_null() {
        drop(Box::from_raw(stream));
    }
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_string_free(value: *mut c_char) {
    if !value.is_null() {
        drop(CString::from_raw(value));
    }
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_bytes_free(value: *mut u8, len: usize) {
    if !value.is_null() {
        drop(Vec::from_raw_parts(value, len, len));
    }
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_sample_free(sample: *mut zgrpc_sample_t) {
    if let Some(sample) = sample.as_mut() {
        zgrpc_string_free(sample.key_expr);
        zgrpc_bytes_free(sample.payload, sample.payload_len);
        clear_sample(sample);
    }
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_query_free(query: *mut zgrpc_query_t) {
    if let Some(query) = query.as_mut() {
        zgrpc_string_free(query.key_expr);
        zgrpc_string_free(query.parameters);
        zgrpc_bytes_free(query.payload, query.payload_len);
        clear_query(query);
    }
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_reply_free(reply: *mut zgrpc_reply_t) {
    if let Some(reply) = reply.as_mut() {
        zgrpc_string_free(reply.key_expr);
        zgrpc_bytes_free(reply.payload, reply.payload_len);
        clear_reply(reply);
    }
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_source_info_free(source_info: *mut zgrpc_source_info_t) {
    if let Some(source_info) = source_info.as_mut() {
        zgrpc_string_free(source_info.id);
        clear_source_info(source_info);
    }
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_sample_ex_free(sample: *mut zgrpc_sample_ex_t) {
    if let Some(sample) = sample.as_mut() {
        zgrpc_string_free(sample.key_expr);
        zgrpc_bytes_free(sample.payload, sample.payload_len);
        zgrpc_string_free(sample.encoding);
        zgrpc_bytes_free(sample.attachment, sample.attachment_len);
        zgrpc_string_free(sample.timestamp);
        zgrpc_source_info_free(&mut sample.source_info);
        clear_sample_ex(sample);
    }
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_subscriber_event_free(event: *mut zgrpc_subscriber_event_t) {
    if let Some(event) = event.as_mut() {
        zgrpc_sample_ex_free(&mut event.sample);
        clear_subscriber_event(event);
    }
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_query_ex_free(query: *mut zgrpc_query_ex_t) {
    if let Some(query) = query.as_mut() {
        zgrpc_string_free(query.selector);
        zgrpc_string_free(query.key_expr);
        zgrpc_string_free(query.parameters);
        zgrpc_bytes_free(query.payload, query.payload_len);
        zgrpc_string_free(query.encoding);
        zgrpc_bytes_free(query.attachment, query.attachment_len);
        clear_query_ex(query);
    }
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_reply_error_free(error: *mut zgrpc_reply_error_t) {
    if let Some(error) = error.as_mut() {
        zgrpc_bytes_free(error.payload, error.payload_len);
        zgrpc_string_free(error.encoding);
        clear_reply_error(error);
    }
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_reply_ex_free(reply: *mut zgrpc_reply_ex_t) {
    if let Some(reply) = reply.as_mut() {
        zgrpc_sample_ex_free(&mut reply.sample);
        zgrpc_reply_error_free(&mut reply.error);
        clear_reply_ex(reply);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_endpoint_accepts_tcp_and_unix() {
        assert!(matches!(
            parse_endpoint(Some("tcp://127.0.0.1:7335")),
            Some(ConnectAddr::Tcp(_))
        ));
        assert!(matches!(
            parse_endpoint(Some("unix:///tmp/zenoh-grpc.sock")),
            Some(ConnectAddr::Unix(_))
        ));
        assert!(parse_endpoint(Some("http://127.0.0.1:7335")).is_none());
    }

    #[test]
    fn default_helpers_zero_initialize_options() {
        unsafe {
            let mut put = mem::MaybeUninit::<zgrpc_session_put_options_t>::uninit();
            zgrpc_session_put_options_default(put.as_mut_ptr());
            let put = put.assume_init();
            assert!(put.encoding.is_null());
            assert_eq!(put.priority, 0);

            let mut query = mem::MaybeUninit::<zgrpc_query_reply_options_t>::uninit();
            zgrpc_query_reply_options_default(query.as_mut_ptr());
            let query = query.assume_init();
            assert!(query.timestamp.is_null());
            assert_eq!(query.attachment_len, 0);
        }
    }
}
