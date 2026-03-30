use std::{
    ffi::{CStr, CString},
    os::raw::{c_char, c_int},
    path::PathBuf,
    ptr,
    sync::Arc,
};

use tokio::runtime::Runtime;
use zenoh_grpc_client_rs::{
    ConnectAddr, DeclarePublisherArgs, DeclareQuerierArgs, DeclareQueryableArgs,
    DeclareSubscriberArgs, GrpcPublisher, GrpcQuerier, GrpcQuery, GrpcQueryable, GrpcSession,
    GrpcSubscriber, PublisherDeleteArgs, PublisherPutArgs, QuerierGetArgs, QueryReplyArgs,
    QueryReplyDeleteArgs, QueryReplyErrArgs, SessionDeleteArgs, SessionPutArgs,
};
use zenoh_grpc_proto::v1 as pb;

fn runtime() -> Arc<Runtime> {
    static RT: std::sync::OnceLock<Arc<Runtime>> = std::sync::OnceLock::new();
    RT.get_or_init(|| Arc::new(Runtime::new().expect("tokio runtime")))
        .clone()
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
}

#[repr(C)]
pub struct zgrpc_queryable_t {
    rt: Arc<Runtime>,
    inner: GrpcQueryable,
}

#[repr(C)]
pub struct zgrpc_querier_t {
    rt: Arc<Runtime>,
    inner: GrpcQuerier,
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

unsafe fn cstr(ptr: *const c_char) -> String {
    CStr::from_ptr(ptr).to_string_lossy().into_owned()
}

fn c_string(value: String) -> *mut c_char {
    CString::new(value).unwrap().into_raw()
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

fn status(result: Result<(), impl std::fmt::Display>) -> c_int {
    match result {
        Ok(()) => 0,
        Err(_) => -1,
    }
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_open_tcp(addr: *const c_char) -> *mut zgrpc_session_t {
    let rt = runtime();
    match rt.block_on(GrpcSession::connect(ConnectAddr::Tcp(cstr(addr)))) {
        Ok(inner) => Box::into_raw(Box::new(zgrpc_session_t { rt, inner })),
        Err(_) => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_open_unix(path: *const c_char) -> *mut zgrpc_session_t {
    let rt = runtime();
    match rt.block_on(GrpcSession::connect(ConnectAddr::Unix(PathBuf::from(
        cstr(path),
    )))) {
        Ok(inner) => Box::into_raw(Box::new(zgrpc_session_t { rt, inner })),
        Err(_) => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_close(session: *mut zgrpc_session_t) {
    if !session.is_null() {
        drop(Box::from_raw(session));
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
        sample.key_expr = ptr::null_mut();
        sample.payload = ptr::null_mut();
        sample.payload_len = 0;
    }
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_query_free(query: *mut zgrpc_query_t) {
    if let Some(query) = query.as_mut() {
        zgrpc_string_free(query.key_expr);
        zgrpc_string_free(query.parameters);
        zgrpc_bytes_free(query.payload, query.payload_len);
        query.key_expr = ptr::null_mut();
        query.parameters = ptr::null_mut();
        query.payload = ptr::null_mut();
        query.payload_len = 0;
        query.query_id = 0;
    }
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_reply_free(reply: *mut zgrpc_reply_t) {
    if let Some(reply) = reply.as_mut() {
        zgrpc_string_free(reply.key_expr);
        zgrpc_bytes_free(reply.payload, reply.payload_len);
        reply.key_expr = ptr::null_mut();
        reply.payload = ptr::null_mut();
        reply.payload_len = 0;
        reply.is_error = 0;
    }
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_session_put(
    session: *mut zgrpc_session_t,
    key_expr: *const c_char,
    payload: *const u8,
    payload_len: usize,
) -> c_int {
    let Some(session) = take_session(session) else {
        return -1;
    };
    let key_expr = cstr(key_expr);
    let payload = std::slice::from_raw_parts(payload, payload_len).to_vec();
    status(session.rt.block_on(session.inner.put(SessionPutArgs {
        key_expr,
        payload,
        ..Default::default()
    })))
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_session_delete(
    session: *mut zgrpc_session_t,
    key_expr: *const c_char,
) -> c_int {
    let Some(session) = take_session(session) else {
        return -1;
    };
    status(session.rt.block_on(session.inner.delete(SessionDeleteArgs {
        key_expr: cstr(key_expr),
        ..Default::default()
    })))
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_declare_publisher(
    session: *mut zgrpc_session_t,
    key_expr: *const c_char,
) -> *mut zgrpc_publisher_t {
    let Some(session) = take_session(session) else {
        return ptr::null_mut();
    };
    match session
        .rt
        .block_on(session.inner.declare_publisher(DeclarePublisherArgs {
            key_expr: cstr(key_expr),
            ..Default::default()
        })) {
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
    let Some(publisher) = take_publisher(publisher) else {
        return -1;
    };
    let payload = std::slice::from_raw_parts(payload, payload_len).to_vec();
    status(publisher.rt.block_on(publisher.inner.put(PublisherPutArgs {
        payload,
        ..Default::default()
    })))
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_publisher_delete(publisher: *mut zgrpc_publisher_t) -> c_int {
    let Some(publisher) = take_publisher(publisher) else {
        return -1;
    };
    status(
        publisher
            .rt
            .block_on(publisher.inner.delete(PublisherDeleteArgs::default())),
    )
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
    let Some(session) = take_session(session) else {
        return ptr::null_mut();
    };
    match session.rt.block_on(session.inner.declare_subscriber(
        DeclareSubscriberArgs {
            key_expr: cstr(key_expr),
            ..Default::default()
        },
        None,
    )) {
        Ok(inner) => Box::into_raw(Box::new(zgrpc_subscriber_t {
            rt: session.rt.clone(),
            inner,
        })),
        Err(_) => ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_declare_queryable(
    session: *mut zgrpc_session_t,
    key_expr: *const c_char,
) -> *mut zgrpc_queryable_t {
    let Some(session) = take_session(session) else {
        return ptr::null_mut();
    };
    match session.rt.block_on(session.inner.declare_queryable(
        DeclareQueryableArgs {
            key_expr: cstr(key_expr),
            ..Default::default()
        },
        None,
    )) {
        Ok(inner) => Box::into_raw(Box::new(zgrpc_queryable_t {
            rt: session.rt.clone(),
            inner,
        })),
        Err(_) => ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_declare_querier(
    session: *mut zgrpc_session_t,
    key_expr: *const c_char,
) -> *mut zgrpc_querier_t {
    let Some(session) = take_session(session) else {
        return ptr::null_mut();
    };
    match session
        .rt
        .block_on(session.inner.declare_querier(DeclareQuerierArgs {
            key_expr: cstr(key_expr),
            ..Default::default()
        })) {
        Ok(inner) => Box::into_raw(Box::new(zgrpc_querier_t {
            rt: session.rt.clone(),
            inner,
        })),
        Err(_) => ptr::null_mut(),
    }
}

fn fill_sample(dst: *mut zgrpc_sample_t, sample: &pb::Sample) -> c_int {
    unsafe {
        let Some(dst) = dst.as_mut() else {
            return -1;
        };
        let mut payload = sample.payload.clone();
        let len = payload.len();
        let ptr = payload.as_mut_ptr();
        std::mem::forget(payload);
        dst.key_expr = c_string(sample.key_expr.clone());
        dst.payload = ptr;
        dst.payload_len = len;
        0
    }
}

fn fill_query(dst: *mut zgrpc_query_t, query: &GrpcQuery) -> c_int {
    unsafe {
        let Some(dst) = dst.as_mut() else {
            return -1;
        };
        let mut payload = query.payload().to_vec();
        let len = payload.len();
        let ptr = payload.as_mut_ptr();
        std::mem::forget(payload);
        dst.query_id = query.query_id();
        dst.key_expr = c_string(query.key_expr().to_string());
        dst.parameters = c_string(query.parameters().to_string());
        dst.payload = ptr;
        dst.payload_len = len;
        0
    }
}

fn fill_reply(dst: *mut zgrpc_reply_t, reply: &pb::Reply) -> c_int {
    unsafe {
        let Some(dst) = dst.as_mut() else {
            return -1;
        };
        match &reply.result {
            Some(pb::reply::Result::Sample(sample)) => {
                let mut payload = sample.payload.clone();
                let len = payload.len();
                let ptr = payload.as_mut_ptr();
                std::mem::forget(payload);
                dst.is_error = 0;
                dst.key_expr = c_string(sample.key_expr.clone());
                dst.payload = ptr;
                dst.payload_len = len;
                0
            }
            Some(pb::reply::Result::Error(err)) => {
                let mut payload = err.payload.clone();
                let len = payload.len();
                let ptr = payload.as_mut_ptr();
                std::mem::forget(payload);
                dst.is_error = 1;
                dst.key_expr = ptr::null_mut();
                dst.payload = ptr;
                dst.payload_len = len;
                0
            }
            None => -1,
        }
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
    match subscriber.inner.recv() {
        Ok(event) => match event.sample {
            Some(sample) => fill_sample(sample_out, &sample),
            None => -1,
        },
        Err(_) => -1,
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
    match subscriber.inner.try_recv() {
        Ok(Some(event)) => match event.sample {
            Some(sample) => fill_sample(sample_out, &sample),
            None => -1,
        },
        Ok(None) => 1,
        Err(_) => -1,
    }
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
pub unsafe extern "C" fn zgrpc_queryable_recv(
    queryable: *mut zgrpc_queryable_t,
    query_out: *mut zgrpc_query_t,
) -> c_int {
    let Some(queryable) = take_queryable(queryable) else {
        return -1;
    };
    match queryable.inner.recv() {
        Ok(query) => fill_query(query_out, &query),
        Err(_) => -1,
    }
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_queryable_reply(
    queryable: *mut zgrpc_queryable_t,
    query_id: u64,
    key_expr: *const c_char,
    payload: *const u8,
    payload_len: usize,
) -> c_int {
    let Some(queryable) = take_queryable(queryable) else {
        return -1;
    };
    let payload = std::slice::from_raw_parts(payload, payload_len).to_vec();
    status(queryable.rt.block_on(queryable.inner.reply(QueryReplyArgs {
        query_id,
        key_expr: cstr(key_expr),
        payload,
        ..Default::default()
    })))
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_queryable_reply_err(
    queryable: *mut zgrpc_queryable_t,
    query_id: u64,
    payload: *const u8,
    payload_len: usize,
) -> c_int {
    let Some(queryable) = take_queryable(queryable) else {
        return -1;
    };
    let payload = std::slice::from_raw_parts(payload, payload_len).to_vec();
    status(
        queryable
            .rt
            .block_on(queryable.inner.reply_err(QueryReplyErrArgs {
                query_id,
                payload,
                ..Default::default()
            })),
    )
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_queryable_reply_delete(
    queryable: *mut zgrpc_queryable_t,
    query_id: u64,
    key_expr: *const c_char,
) -> c_int {
    let Some(queryable) = take_queryable(queryable) else {
        return -1;
    };
    status(
        queryable
            .rt
            .block_on(queryable.inner.reply_delete(QueryReplyDeleteArgs {
                query_id,
                key_expr: cstr(key_expr),
                ..Default::default()
            })),
    )
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_queryable_undeclare(queryable: *mut zgrpc_queryable_t) -> c_int {
    if queryable.is_null() {
        return -1;
    }
    let queryable = Box::from_raw(queryable);
    status(queryable.rt.block_on(queryable.inner.undeclare()))
}

#[no_mangle]
pub unsafe extern "C" fn zgrpc_querier_get(
    querier: *mut zgrpc_querier_t,
    parameters: *const c_char,
    reply_out: *mut zgrpc_reply_t,
) -> c_int {
    let Some(querier) = take_querier(querier) else {
        return -1;
    };
    let parameters = if parameters.is_null() {
        String::new()
    } else {
        cstr(parameters)
    };
    match querier.rt.block_on(querier.inner.get(QuerierGetArgs {
        parameters,
        ..Default::default()
    })) {
        Ok(replies) => match replies.recv() {
            Ok(reply) => fill_reply(reply_out, &reply),
            Err(_) => -1,
        },
        Err(_) => -1,
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
