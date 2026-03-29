use std::sync::Arc;

use pyo3::prelude::*;
use tokio::runtime::Runtime;
use zenoh_grpc_client_rs::{
    DeclarePublisherArgs as RsDeclarePublisherArgs, DeclareQuerierArgs as RsDeclareQuerierArgs,
    DeclareQueryableArgs as RsDeclareQueryableArgs,
    DeclareSubscriberArgs as RsDeclareSubscriberArgs, GrpcPublisher, GrpcQuerier, GrpcQueryable,
    GrpcSession, GrpcSubscriber, PublisherDeleteArgs as RsPublisherDeleteArgs,
    PublisherPutArgs as RsPublisherPutArgs, QuerierGetArgs as RsQuerierGetArgs,
    QueryableCallback as RsQueryableCallback, SessionDeleteArgs as RsSessionDeleteArgs,
    SessionGetArgs as RsSessionGetArgs, SessionPutArgs as RsSessionPutArgs,
    SubscriberCallback as RsSubscriberCallback,
};

use crate::{
    common::{parse_connect_addr, runtime, to_py_err},
    events::{Query, QueryStream, ReplyStream, SubscriberEvent},
};

#[pyclass]
pub(crate) struct Session {
    rt: Arc<Runtime>,
    inner: GrpcSession,
}

#[pyclass]
pub(crate) struct Publisher {
    rt: Arc<Runtime>,
    inner: GrpcPublisher,
}

#[pyclass]
pub(crate) struct Subscriber {
    rt: Arc<Runtime>,
    inner: GrpcSubscriber,
}

#[pyclass]
pub(crate) struct Queryable {
    rt: Arc<Runtime>,
    inner: GrpcQueryable,
}

#[pyclass]
pub(crate) struct Querier {
    rt: Arc<Runtime>,
    inner: GrpcQuerier,
}

#[pymethods]
impl Session {
    fn __enter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __exit__(
        &self,
        py: Python<'_>,
        _exc_type: Option<&Bound<'_, PyAny>>,
        _exc: Option<&Bound<'_, PyAny>>,
        _tb: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        self.close(py)
    }

    #[staticmethod]
    #[pyo3(signature = (endpoint = "unix:///tmp/zenoh-grpc.sock".to_string()))]
    fn connect(py: Python<'_>, endpoint: String) -> PyResult<Self> {
        let rt = runtime();
        let addr = parse_connect_addr(&endpoint)?;
        let inner = py
            .allow_threads(|| rt.block_on(GrpcSession::connect(addr)))
            .map_err(to_py_err)?;
        Ok(Self { rt, inner })
    }

    fn info(&self, py: Python<'_>) -> PyResult<String> {
        Ok(py
            .allow_threads(|| self.rt.block_on(self.inner.info()))
            .map_err(to_py_err)?
            .zid)
    }

    fn close(&self, py: Python<'_>) -> PyResult<()> {
        py.allow_threads(|| self.rt.block_on(self.inner.cleanup()))
            .map_err(to_py_err)
    }

    #[pyo3(signature = (key_expr, payload, encoding=None, congestion_control=None, priority=None, express=false, attachment=None, timestamp=None, allowed_destination=None))]
    fn put(
        &self,
        py: Python<'_>,
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
        py.allow_threads(|| {
            self.rt.block_on(self.inner.put(RsSessionPutArgs {
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
        })
        .map_err(to_py_err)
    }

    #[pyo3(signature = (key_expr, congestion_control=None, priority=None, express=false, attachment=None, timestamp=None, allowed_destination=None))]
    fn delete(
        &self,
        py: Python<'_>,
        key_expr: String,
        congestion_control: Option<i32>,
        priority: Option<i32>,
        express: bool,
        attachment: Option<Vec<u8>>,
        timestamp: Option<String>,
        allowed_destination: Option<i32>,
    ) -> PyResult<()> {
        py.allow_threads(|| {
            self.rt.block_on(self.inner.delete(RsSessionDeleteArgs {
                key_expr,
                congestion_control: congestion_control.unwrap_or_default(),
                priority: priority.unwrap_or_default(),
                express,
                attachment: attachment.unwrap_or_default(),
                timestamp: timestamp.unwrap_or_default(),
                allowed_destination: allowed_destination.unwrap_or_default(),
            }))
        })
        .map_err(to_py_err)
    }

    #[pyo3(signature = (selector, target=None, consolidation=None, timeout_ms=None, payload=None, encoding=None, attachment=None, allowed_destination=None))]
    fn get(
        &self,
        py: Python<'_>,
        selector: String,
        target: Option<i32>,
        consolidation: Option<i32>,
        timeout_ms: Option<u64>,
        payload: Option<Vec<u8>>,
        encoding: Option<String>,
        attachment: Option<Vec<u8>>,
        allowed_destination: Option<i32>,
    ) -> PyResult<ReplyStream> {
        let replies = py
            .allow_threads(|| {
                self.rt.block_on(self.inner.get(RsSessionGetArgs {
                    selector,
                    target: target.unwrap_or_default(),
                    consolidation: consolidation.unwrap_or_default(),
                    timeout_ms: timeout_ms.unwrap_or_default(),
                    payload: payload.unwrap_or_default(),
                    encoding: encoding.unwrap_or_default(),
                    attachment: attachment.unwrap_or_default(),
                    allowed_destination: allowed_destination.unwrap_or_default(),
                }))
            })
            .map_err(to_py_err)?;
        Ok(ReplyStream::from_inner(replies))
    }

    #[pyo3(signature = (key_expr, encoding=None, congestion_control=None, priority=None, express=false, reliability=None, allowed_destination=None))]
    fn declare_publisher(
        &self,
        py: Python<'_>,
        key_expr: String,
        encoding: Option<String>,
        congestion_control: Option<i32>,
        priority: Option<i32>,
        express: bool,
        reliability: Option<i32>,
        allowed_destination: Option<i32>,
    ) -> PyResult<Publisher> {
        let inner = py
            .allow_threads(|| {
                self.rt
                    .block_on(self.inner.declare_publisher(RsDeclarePublisherArgs {
                        key_expr,
                        encoding: encoding.unwrap_or_default(),
                        congestion_control: congestion_control.unwrap_or_default(),
                        priority: priority.unwrap_or_default(),
                        express,
                        reliability: reliability.unwrap_or_default(),
                        allowed_destination: allowed_destination.unwrap_or_default(),
                    }))
            })
            .map_err(to_py_err)?;
        Ok(Publisher {
            rt: self.rt.clone(),
            inner,
        })
    }

    #[pyo3(signature = (key_expr, callback=None, allowed_origin=None))]
    fn declare_subscriber(
        &self,
        py: Python<'_>,
        key_expr: String,
        callback: Option<Py<PyAny>>,
        allowed_origin: Option<i32>,
    ) -> PyResult<Subscriber> {
        let callback: Option<RsSubscriberCallback> = callback.map(|callback| {
            Arc::new(move |inner| {
                Python::with_gil(|py| {
                    if let Err(err) = callback.call1(py, (SubscriberEvent { inner },)) {
                        err.print(py);
                    }
                });
            }) as RsSubscriberCallback
        });
        let inner = py
            .allow_threads(|| {
                self.rt.block_on(self.inner.declare_subscriber(
                    RsDeclareSubscriberArgs {
                        key_expr,
                        allowed_origin: allowed_origin.unwrap_or_default(),
                    },
                    callback,
                ))
            })
            .map_err(to_py_err)?;
        Ok(Subscriber {
            rt: self.rt.clone(),
            inner,
        })
    }

    #[pyo3(signature = (key_expr, callback=None, complete=false, allowed_origin=None))]
    fn declare_queryable(
        &self,
        py: Python<'_>,
        key_expr: String,
        callback: Option<Py<PyAny>>,
        complete: bool,
        allowed_origin: Option<i32>,
    ) -> PyResult<Queryable> {
        let rt = self.rt.clone();
        let callback: Option<RsQueryableCallback> = callback.map(|callback| {
            let rt = rt.clone();
            Arc::new(move |inner| {
                Python::with_gil(|py| {
                    if let Err(err) = callback.call1(py, (Query::from_inner(rt.clone(), inner),)) {
                        err.print(py);
                    }
                });
            }) as RsQueryableCallback
        });
        let inner = py
            .allow_threads(|| {
                self.rt.block_on(self.inner.declare_queryable(
                    RsDeclareQueryableArgs {
                        key_expr,
                        complete,
                        allowed_origin: allowed_origin.unwrap_or_default(),
                    },
                    callback,
                ))
            })
            .map_err(to_py_err)?;
        Ok(Queryable {
            rt: self.rt.clone(),
            inner,
        })
    }

    #[pyo3(signature = (key_expr, target=None, consolidation=None, timeout_ms=None, allowed_destination=None))]
    fn declare_querier(
        &self,
        py: Python<'_>,
        key_expr: String,
        target: Option<i32>,
        consolidation: Option<i32>,
        timeout_ms: Option<u64>,
        allowed_destination: Option<i32>,
    ) -> PyResult<Querier> {
        let inner = py
            .allow_threads(|| {
                self.rt
                    .block_on(self.inner.declare_querier(RsDeclareQuerierArgs {
                        key_expr,
                        target: target.unwrap_or_default(),
                        consolidation: consolidation.unwrap_or_default(),
                        timeout_ms: timeout_ms.unwrap_or_default(),
                        allowed_destination: allowed_destination.unwrap_or_default(),
                    }))
            })
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
        py: Python<'_>,
        _exc_type: Option<&Bound<'_, PyAny>>,
        _exc: Option<&Bound<'_, PyAny>>,
        _tb: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        self.undeclare(py)
    }

    #[pyo3(signature = (payload, encoding=None, attachment=None, timestamp=None))]
    fn put(
        &self,
        py: Python<'_>,
        payload: Vec<u8>,
        encoding: Option<String>,
        attachment: Option<Vec<u8>>,
        timestamp: Option<String>,
    ) -> PyResult<()> {
        py.allow_threads(|| {
            self.rt.block_on(self.inner.put(RsPublisherPutArgs {
                payload,
                encoding: encoding.unwrap_or_default(),
                attachment: attachment.unwrap_or_default(),
                timestamp: timestamp.unwrap_or_default(),
            }))
        })
        .map_err(to_py_err)
    }

    #[pyo3(signature = (attachment=None, timestamp=None))]
    fn delete(
        &self,
        py: Python<'_>,
        attachment: Option<Vec<u8>>,
        timestamp: Option<String>,
    ) -> PyResult<()> {
        py.allow_threads(|| {
            self.rt.block_on(self.inner.delete(RsPublisherDeleteArgs {
                attachment: attachment.unwrap_or_default(),
                timestamp: timestamp.unwrap_or_default(),
            }))
        })
        .map_err(to_py_err)
    }

    fn undeclare(&self, py: Python<'_>) -> PyResult<()> {
        py.allow_threads(|| self.rt.block_on(self.inner.undeclare()))
            .map_err(to_py_err)
    }

    fn send_dropped_count(&self) -> u64 {
        self.inner.send_dropped_count()
    }
}

#[pymethods]
impl Subscriber {
    fn __enter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __exit__(
        &self,
        py: Python<'_>,
        _exc_type: Option<&Bound<'_, PyAny>>,
        _exc: Option<&Bound<'_, PyAny>>,
        _tb: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        self.undeclare(py)
    }

    fn recv(&self, py: Python<'_>) -> PyResult<SubscriberEvent> {
        py.allow_threads(|| self.inner.recv())
            .map(|inner| SubscriberEvent { inner })
            .map_err(to_py_err)
    }

    fn try_recv(&self) -> PyResult<Option<SubscriberEvent>> {
        self.inner
            .try_recv()
            .map(|event| event.map(|inner| SubscriberEvent { inner }))
            .map_err(to_py_err)
    }

    fn undeclare(&self, py: Python<'_>) -> PyResult<()> {
        py.allow_threads(|| self.rt.block_on(self.inner.undeclare()))
            .map_err(to_py_err)
    }

    fn dropped_count(&self) -> u64 {
        self.inner.dropped_count()
    }
}

#[pymethods]
impl Queryable {
    fn __enter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __exit__(
        &self,
        py: Python<'_>,
        _exc_type: Option<&Bound<'_, PyAny>>,
        _exc: Option<&Bound<'_, PyAny>>,
        _tb: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        self.undeclare(py)
    }

    fn receiver(&self) -> PyResult<QueryStream> {
        let receiver = self.inner.receiver().map_err(to_py_err)?;
        Ok(QueryStream::from_inner(self.rt.clone(), receiver))
    }

    fn undeclare(&self, py: Python<'_>) -> PyResult<()> {
        py.allow_threads(|| self.rt.block_on(self.inner.undeclare()))
            .map_err(to_py_err)
    }

    fn send_dropped_count(&self) -> u64 {
        self.inner.send_dropped_count()
    }
}

#[pymethods]
impl Querier {
    fn __enter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __exit__(
        &self,
        py: Python<'_>,
        _exc_type: Option<&Bound<'_, PyAny>>,
        _exc: Option<&Bound<'_, PyAny>>,
        _tb: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        self.undeclare(py)
    }

    #[pyo3(signature = (parameters=None, payload=None, encoding=None, attachment=None))]
    fn get(
        &self,
        py: Python<'_>,
        parameters: Option<String>,
        payload: Option<Vec<u8>>,
        encoding: Option<String>,
        attachment: Option<Vec<u8>>,
    ) -> PyResult<ReplyStream> {
        let replies = py
            .allow_threads(|| {
                self.rt.block_on(self.inner.get(RsQuerierGetArgs {
                    parameters: parameters.unwrap_or_default(),
                    payload: payload.unwrap_or_default(),
                    encoding: encoding.unwrap_or_default(),
                    attachment: attachment.unwrap_or_default(),
                }))
            })
            .map_err(to_py_err)?;
        Ok(ReplyStream::from_inner(replies))
    }

    fn undeclare(&self, py: Python<'_>) -> PyResult<()> {
        py.allow_threads(|| self.rt.block_on(self.inner.undeclare()))
            .map_err(to_py_err)
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<Session>()?;
    module.add_class::<Publisher>()?;
    module.add_class::<Subscriber>()?;
    module.add_class::<Queryable>()?;
    module.add_class::<Querier>()?;
    Ok(())
}
