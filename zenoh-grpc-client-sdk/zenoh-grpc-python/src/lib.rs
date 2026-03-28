use std::{path::PathBuf, sync::Arc, thread};

use pyo3::{exceptions::PyRuntimeError, prelude::*};
use tokio::runtime::Runtime;
use zenoh_grpc_client_rs::{
    ConnectAddr, DeclarePublisherArgs as RsDeclarePublisherArgs,
    DeclareQuerierArgs as RsDeclareQuerierArgs,
    DeclareQueryableArgs as RsDeclareQueryableArgs,
    DeclareSubscriberArgs as RsDeclareSubscriberArgs, GrpcPublisher, GrpcQuerier, GrpcQueryable,
    GrpcSession, GrpcSubscriber, PublisherDeleteArgs as RsPublisherDeleteArgs,
    PublisherPutArgs as RsPublisherPutArgs, QuerierGetArgs as RsQuerierGetArgs,
    QueryReplyArgs as RsQueryReplyArgs, QueryReplyDeleteArgs as RsQueryReplyDeleteArgs,
    QueryReplyErrArgs as RsQueryReplyErrArgs, SessionDeleteArgs as RsSessionDeleteArgs,
    SessionGetArgs as RsSessionGetArgs, SessionPutArgs as RsSessionPutArgs,
};
use zenoh_grpc_proto::v1 as pb;

fn runtime() -> Arc<Runtime> {
    static RT: std::sync::OnceLock<Arc<Runtime>> = std::sync::OnceLock::new();
    RT.get_or_init(|| Arc::new(Runtime::new().expect("tokio runtime"))).clone()
}

fn to_py_err<E: std::fmt::Display>(err: E) -> PyErr {
    PyRuntimeError::new_err(err.to_string())
}

fn parse_connect_addr(endpoint: &str) -> PyResult<ConnectAddr> {
    if let Some(addr) = endpoint.strip_prefix("tcp://") {
        return Ok(ConnectAddr::Tcp(addr.to_string()));
    }
    if let Some(path) = endpoint.strip_prefix("unix://") {
        return Ok(ConnectAddr::Unix(PathBuf::from(path)));
    }
    Err(PyRuntimeError::new_err(
        "invalid endpoint, expected tcp://host:port or unix:///path",
    ))
}

fn collect_replies(replies: flume::Receiver<pb::Reply>) -> Vec<String> {
    let mut values = Vec::new();
    while let Ok(reply) = replies.recv() {
        if let Some(value) = format_reply(&reply) {
            values.push(value);
        }
    }
    values
}

#[pyclass]
struct Session {
    rt: Arc<Runtime>,
    inner: GrpcSession,
}

#[pyclass]
struct Publisher {
    rt: Arc<Runtime>,
    inner: GrpcPublisher,
}

#[pyclass]
struct Subscriber {
    rt: Arc<Runtime>,
    inner: GrpcSubscriber,
}

#[pyclass]
struct Queryable {
    rt: Arc<Runtime>,
    inner: GrpcQueryable,
}

#[pyclass]
struct Querier {
    rt: Arc<Runtime>,
    inner: GrpcQuerier,
}

#[pyclass]
#[derive(Clone)]
struct SubscriberEvent {
    inner: pb::SubscriberEvent,
}

#[pyclass]
#[derive(Clone)]
struct QueryableEvent {
    inner: pb::QueryableEvent,
}

#[pymethods]
impl Session {
    fn __enter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __exit__(
        &self,
        _exc_type: Option<&Bound<'_, PyAny>>,
        _exc: Option<&Bound<'_, PyAny>>,
        _tb: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        self.close()
    }

    #[staticmethod]
    #[pyo3(signature = (endpoint = "tcp://127.0.0.1:7335".to_string()))]
    fn connect(endpoint: String) -> PyResult<Self> {
        let rt = runtime();
        let inner = rt
            .block_on(GrpcSession::connect(parse_connect_addr(&endpoint)?))
            .map_err(to_py_err)?;
        Ok(Self { rt, inner })
    }

    fn info(&self) -> PyResult<String> {
        Ok(self.rt.block_on(self.inner.info()).map_err(to_py_err)?.zid)
    }

    fn close(&self) -> PyResult<()> {
        self.rt.block_on(self.inner.cleanup()).map_err(to_py_err)
    }

    #[pyo3(signature = (key_expr, payload, encoding=None, congestion_control=None, priority=None, express=false, attachment=None, timestamp=None, allowed_destination=None))]
    fn put(
        &self,
        key_expr: String,
        payload: Vec<u8>,
        encoding: Option<String>,
        congestion_control: Option<i32>,
        priority: Option<i32>,
        express: bool,
        attachment: Option<Vec<u8>>,
        timestamp: Option<String>,
        allowed_destination: Option<i32>,
    ) -> PyResult<()> {
        self.rt
            .block_on(self.inner.put(RsSessionPutArgs {
                key_expr,
                payload,
                encoding: encoding.unwrap_or_default(),
                congestion_control: congestion_control.unwrap_or_default(),
                priority: priority.unwrap_or_default(),
                express,
                attachment: attachment.unwrap_or_default(),
                timestamp: timestamp.unwrap_or_default(),
                allowed_destination: allowed_destination.unwrap_or_default(),
            }))
            .map_err(to_py_err)
    }

    #[pyo3(signature = (key_expr, congestion_control=None, priority=None, express=false, attachment=None, timestamp=None, allowed_destination=None))]
    fn delete(
        &self,
        key_expr: String,
        congestion_control: Option<i32>,
        priority: Option<i32>,
        express: bool,
        attachment: Option<Vec<u8>>,
        timestamp: Option<String>,
        allowed_destination: Option<i32>,
    ) -> PyResult<()> {
        self.rt
            .block_on(self.inner.delete(RsSessionDeleteArgs {
                key_expr,
                congestion_control: congestion_control.unwrap_or_default(),
                priority: priority.unwrap_or_default(),
                express,
                attachment: attachment.unwrap_or_default(),
                timestamp: timestamp.unwrap_or_default(),
                allowed_destination: allowed_destination.unwrap_or_default(),
            }))
            .map_err(to_py_err)
    }

    #[pyo3(signature = (selector, target=None, consolidation=None, timeout_ms=None, payload=None, encoding=None, attachment=None, allowed_destination=None))]
    fn get(
        &self,
        selector: String,
        target: Option<i32>,
        consolidation: Option<i32>,
        timeout_ms: Option<u64>,
        payload: Option<Vec<u8>>,
        encoding: Option<String>,
        attachment: Option<Vec<u8>>,
        allowed_destination: Option<i32>,
    ) -> PyResult<Vec<String>> {
        let replies = self
            .rt
            .block_on(self.inner.get(RsSessionGetArgs {
                selector,
                target: target.unwrap_or_default(),
                consolidation: consolidation.unwrap_or_default(),
                timeout_ms: timeout_ms.unwrap_or_default(),
                payload: payload.unwrap_or_default(),
                encoding: encoding.unwrap_or_default(),
                attachment: attachment.unwrap_or_default(),
                allowed_destination: allowed_destination.unwrap_or_default(),
            }))
            .map_err(to_py_err)?;
        Ok(collect_replies(replies))
    }

    #[pyo3(signature = (key_expr, encoding=None, congestion_control=None, priority=None, express=false, reliability=None, allowed_destination=None))]
    fn declare_publisher(
        &self,
        key_expr: String,
        encoding: Option<String>,
        congestion_control: Option<i32>,
        priority: Option<i32>,
        express: bool,
        reliability: Option<i32>,
        allowed_destination: Option<i32>,
    ) -> PyResult<Publisher> {
        let inner = self
            .rt
            .block_on(self.inner.declare_publisher(RsDeclarePublisherArgs {
                key_expr,
                encoding: encoding.unwrap_or_default(),
                congestion_control: congestion_control.unwrap_or_default(),
                priority: priority.unwrap_or_default(),
                express,
                reliability: reliability.unwrap_or_default(),
                allowed_destination: allowed_destination.unwrap_or_default(),
            }))
            .map_err(to_py_err)?;
        Ok(Publisher {
            rt: self.rt.clone(),
            inner,
        })
    }

    #[pyo3(signature = (key_expr, allowed_origin=None))]
    fn declare_subscriber(&self, key_expr: String, allowed_origin: Option<i32>) -> PyResult<Subscriber> {
        let inner = self
            .rt
            .block_on(self.inner.declare_subscriber(RsDeclareSubscriberArgs {
                key_expr,
                allowed_origin: allowed_origin.unwrap_or_default(),
            }))
            .map_err(to_py_err)?;
        Ok(Subscriber {
            rt: self.rt.clone(),
            inner,
        })
    }

    #[pyo3(signature = (key_expr, complete=false, allowed_origin=None))]
    fn declare_queryable(
        &self,
        key_expr: String,
        complete: bool,
        allowed_origin: Option<i32>,
    ) -> PyResult<Queryable> {
        let inner = self
            .rt
            .block_on(self.inner.declare_queryable(RsDeclareQueryableArgs {
                key_expr,
                complete,
                allowed_origin: allowed_origin.unwrap_or_default(),
            }))
            .map_err(to_py_err)?;
        Ok(Queryable {
            rt: self.rt.clone(),
            inner,
        })
    }

    #[pyo3(signature = (key_expr, target=None, consolidation=None, timeout_ms=None, allowed_destination=None))]
    fn declare_querier(
        &self,
        key_expr: String,
        target: Option<i32>,
        consolidation: Option<i32>,
        timeout_ms: Option<u64>,
        allowed_destination: Option<i32>,
    ) -> PyResult<Querier> {
        let inner = self
            .rt
            .block_on(self.inner.declare_querier(RsDeclareQuerierArgs {
                key_expr,
                target: target.unwrap_or_default(),
                consolidation: consolidation.unwrap_or_default(),
                timeout_ms: timeout_ms.unwrap_or_default(),
                allowed_destination: allowed_destination.unwrap_or_default(),
            }))
            .map_err(to_py_err)?;
        Ok(Querier {
            rt: self.rt.clone(),
            inner,
        })
    }
}

#[pymethods]
impl Publisher {
    fn __enter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __exit__(
        &self,
        _exc_type: Option<&Bound<'_, PyAny>>,
        _exc: Option<&Bound<'_, PyAny>>,
        _tb: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        self.undeclare()
    }

    #[pyo3(signature = (payload, encoding=None, attachment=None, timestamp=None))]
    fn put(
        &self,
        payload: Vec<u8>,
        encoding: Option<String>,
        attachment: Option<Vec<u8>>,
        timestamp: Option<String>,
    ) -> PyResult<()> {
        self.rt
            .block_on(self.inner.put(RsPublisherPutArgs {
                payload,
                encoding: encoding.unwrap_or_default(),
                attachment: attachment.unwrap_or_default(),
                timestamp: timestamp.unwrap_or_default(),
            }))
            .map_err(to_py_err)
    }

    #[pyo3(signature = (attachment=None, timestamp=None))]
    fn delete(&self, attachment: Option<Vec<u8>>, timestamp: Option<String>) -> PyResult<()> {
        self.rt
            .block_on(self.inner.delete(RsPublisherDeleteArgs {
                attachment: attachment.unwrap_or_default(),
                timestamp: timestamp.unwrap_or_default(),
            }))
            .map_err(to_py_err)
    }

    fn undeclare(&self) -> PyResult<()> {
        self.rt.block_on(self.inner.undeclare()).map_err(to_py_err)
    }
}

#[pymethods]
impl Subscriber {
    fn __enter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __exit__(
        &self,
        _exc_type: Option<&Bound<'_, PyAny>>,
        _exc: Option<&Bound<'_, PyAny>>,
        _tb: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        self.undeclare()
    }

    fn recv(&self) -> PyResult<SubscriberEvent> {
        self.inner
            .receiver()
            .recv()
            .map(|inner| SubscriberEvent { inner })
            .map_err(to_py_err)
    }

    fn try_recv(&self) -> PyResult<Option<SubscriberEvent>> {
        match self.inner.receiver().try_recv() {
            Ok(inner) => Ok(Some(SubscriberEvent { inner })),
            Err(flume::TryRecvError::Empty) => Ok(None),
            Err(err) => Err(to_py_err(err)),
        }
    }

    fn undeclare(&self) -> PyResult<()> {
        self.rt.block_on(self.inner.undeclare()).map_err(to_py_err)
    }

    fn run(&self, callback: Py<PyAny>) {
        let subscriber = self.inner.clone();
        thread::spawn(move || {
            while let Ok(inner) = subscriber.receiver().recv() {
                Python::with_gil(|py| {
                    let _ = callback.call1(py, (SubscriberEvent { inner },));
                });
            }
        });
    }
}

#[pymethods]
impl Queryable {
    fn __enter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __exit__(
        &self,
        _exc_type: Option<&Bound<'_, PyAny>>,
        _exc: Option<&Bound<'_, PyAny>>,
        _tb: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        self.undeclare()
    }

    fn recv(&self) -> PyResult<QueryableEvent> {
        self.inner
            .receiver()
            .recv()
            .map(|inner| QueryableEvent { inner })
            .map_err(to_py_err)
    }

    #[pyo3(signature = (query_id, key_expr, payload, encoding=None, attachment=None, timestamp=None))]
    fn reply(
        &self,
        query_id: u64,
        key_expr: String,
        payload: Vec<u8>,
        encoding: Option<String>,
        attachment: Option<Vec<u8>>,
        timestamp: Option<String>,
    ) -> PyResult<()> {
        self.rt
            .block_on(self.inner.reply(RsQueryReplyArgs {
                query_id,
                key_expr,
                payload,
                encoding: encoding.unwrap_or_default(),
                attachment: attachment.unwrap_or_default(),
                timestamp: timestamp.unwrap_or_default(),
            }))
            .map_err(to_py_err)
    }

    #[pyo3(signature = (query_id, payload, encoding=None))]
    fn reply_err(&self, query_id: u64, payload: Vec<u8>, encoding: Option<String>) -> PyResult<()> {
        self.rt
            .block_on(self.inner.reply_err(RsQueryReplyErrArgs {
                query_id,
                payload,
                encoding: encoding.unwrap_or_default(),
            }))
            .map_err(to_py_err)
    }

    #[pyo3(signature = (query_id, key_expr, attachment=None, timestamp=None))]
    fn reply_delete(
        &self,
        query_id: u64,
        key_expr: String,
        attachment: Option<Vec<u8>>,
        timestamp: Option<String>,
    ) -> PyResult<()> {
        self.rt
            .block_on(self.inner.reply_delete(RsQueryReplyDeleteArgs {
                query_id,
                key_expr,
                attachment: attachment.unwrap_or_default(),
                timestamp: timestamp.unwrap_or_default(),
            }))
            .map_err(to_py_err)
    }

    fn undeclare(&self) -> PyResult<()> {
        self.rt.block_on(self.inner.undeclare()).map_err(to_py_err)
    }

    fn run(&self, callback: Py<PyAny>) {
        let queryable = self.inner.clone();
        thread::spawn(move || {
            while let Ok(inner) = queryable.receiver().recv() {
                Python::with_gil(|py| {
                    let _ = callback.call1(py, (QueryableEvent { inner },));
                });
            }
        });
    }
}

#[pymethods]
impl Querier {
    fn __enter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __exit__(
        &self,
        _exc_type: Option<&Bound<'_, PyAny>>,
        _exc: Option<&Bound<'_, PyAny>>,
        _tb: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        self.undeclare()
    }

    #[pyo3(signature = (parameters=None, payload=None, encoding=None, attachment=None))]
    fn get(
        &self,
        parameters: Option<String>,
        payload: Option<Vec<u8>>,
        encoding: Option<String>,
        attachment: Option<Vec<u8>>,
    ) -> PyResult<Vec<String>> {
        let replies = self
            .rt
            .block_on(self.inner.get(RsQuerierGetArgs {
                parameters: parameters.unwrap_or_default(),
                payload: payload.unwrap_or_default(),
                encoding: encoding.unwrap_or_default(),
                attachment: attachment.unwrap_or_default(),
            }))
            .map_err(to_py_err)?;
        Ok(collect_replies(replies))
    }

    fn undeclare(&self) -> PyResult<()> {
        self.rt.block_on(self.inner.undeclare()).map_err(to_py_err)
    }
}

#[pymethods]
impl SubscriberEvent {
    #[getter]
    fn key_expr(&self) -> Option<String> {
        self.inner.sample.as_ref().map(|s| s.key_expr.clone())
    }

    #[getter]
    fn payload(&self) -> Option<Vec<u8>> {
        self.inner.sample.as_ref().map(|s| s.payload.clone())
    }
}

#[pymethods]
impl QueryableEvent {
    #[getter]
    fn query_id(&self) -> Option<u64> {
        self.inner.query.as_ref().map(|q| q.query_id)
    }

    #[getter]
    fn key_expr(&self) -> Option<String> {
        self.inner.query.as_ref().map(|q| q.key_expr.clone())
    }

    #[getter]
    fn parameters(&self) -> Option<String> {
        self.inner.query.as_ref().map(|q| q.parameters.clone())
    }

    #[getter]
    fn payload(&self) -> Option<Vec<u8>> {
        self.inner.query.as_ref().map(|q| q.payload.clone())
    }
}

fn format_reply(reply: &pb::Reply) -> Option<String> {
    match &reply.result {
        Some(pb::reply::Result::Sample(sample)) => Some(format!(
            "{}:{}",
            sample.key_expr,
            String::from_utf8_lossy(&sample.payload)
        )),
        Some(pb::reply::Result::Error(err)) => Some(format!(
            "ERROR:{}",
            String::from_utf8_lossy(&err.payload)
        )),
        None => None,
    }
}

#[pymodule]
fn zenoh_grpc(_py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<Session>()?;
    module.add_class::<Publisher>()?;
    module.add_class::<Subscriber>()?;
    module.add_class::<Queryable>()?;
    module.add_class::<Querier>()?;
    module.add_class::<SubscriberEvent>()?;
    module.add_class::<QueryableEvent>()?;
    Ok(())
}
