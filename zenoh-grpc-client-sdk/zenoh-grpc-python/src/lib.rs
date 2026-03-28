use std::{path::PathBuf, sync::Arc};

use pyo3::{exceptions::PyRuntimeError, prelude::*, types::PyIterator};
use tokio::runtime::Runtime;
use zenoh_grpc_client_rs::{ConnectAddr, GrpcPublisher, GrpcQuerier, GrpcQueryable, GrpcSession, GrpcSubscriber};
use zenoh_grpc_proto::v1 as pb;

fn runtime() -> Arc<Runtime> {
    static RT: std::sync::OnceLock<Arc<Runtime>> = std::sync::OnceLock::new();
    RT.get_or_init(|| Arc::new(Runtime::new().expect("tokio runtime"))).clone()
}

fn to_py_err<E: std::fmt::Display>(err: E) -> PyErr {
    PyRuntimeError::new_err(err.to_string())
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
    #[staticmethod]
    fn connect_tcp(addr: String) -> PyResult<Self> {
        let rt = runtime();
        let inner = rt
            .block_on(GrpcSession::connect(ConnectAddr::Tcp(addr)))
            .map_err(to_py_err)?;
        Ok(Self { rt, inner })
    }

    #[staticmethod]
    fn connect_unix(path: String) -> PyResult<Self> {
        let rt = runtime();
        let inner = rt
            .block_on(GrpcSession::connect(ConnectAddr::Unix(PathBuf::from(path))))
            .map_err(to_py_err)?;
        Ok(Self { rt, inner })
    }

    fn info(&self) -> PyResult<String> {
        Ok(self.rt.block_on(self.inner.info()).map_err(to_py_err)?.zid)
    }

    fn put(&self, key_expr: String, payload: Vec<u8>) -> PyResult<()> {
        self.rt.block_on(self.inner.put(key_expr, payload)).map_err(to_py_err)
    }

    fn delete(&self, key_expr: String) -> PyResult<()> {
        self.rt.block_on(self.inner.delete(key_expr)).map_err(to_py_err)
    }

    fn get(&self, py: Python<'_>, selector: String) -> PyResult<PyObject> {
        let replies = self.rt.block_on(self.inner.get(selector)).map_err(to_py_err)?;
        let values: Vec<String> = replies
            .drain()
            .filter_map(|reply| format_reply(&reply))
            .collect();
        Ok(values.into_py(py))
    }

    fn declare_publisher(&self, key_expr: String) -> PyResult<Publisher> {
        let inner = self
            .rt
            .block_on(self.inner.declare_publisher(key_expr))
            .map_err(to_py_err)?;
        Ok(Publisher {
            rt: self.rt.clone(),
            inner,
        })
    }

    fn declare_subscriber(&self, key_expr: String) -> PyResult<Subscriber> {
        let inner = self
            .rt
            .block_on(self.inner.declare_subscriber(key_expr))
            .map_err(to_py_err)?;
        Ok(Subscriber {
            rt: self.rt.clone(),
            inner,
        })
    }

    fn declare_queryable(&self, key_expr: String) -> PyResult<Queryable> {
        let inner = self
            .rt
            .block_on(self.inner.declare_queryable(key_expr))
            .map_err(to_py_err)?;
        Ok(Queryable {
            rt: self.rt.clone(),
            inner,
        })
    }

    fn declare_querier(&self, key_expr: String) -> PyResult<Querier> {
        let inner = self
            .rt
            .block_on(self.inner.declare_querier(key_expr))
            .map_err(to_py_err)?;
        Ok(Querier {
            rt: self.rt.clone(),
            inner,
        })
    }
}

#[pymethods]
impl Publisher {
    fn put(&self, payload: Vec<u8>) -> PyResult<()> {
        self.rt.block_on(self.inner.put(payload)).map_err(to_py_err)
    }

    fn delete(&self) -> PyResult<()> {
        self.rt.block_on(self.inner.delete()).map_err(to_py_err)
    }

    fn undeclare(&self) -> PyResult<()> {
        self.rt.block_on(self.inner.undeclare()).map_err(to_py_err)
    }
}

#[pymethods]
impl Subscriber {
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
}

#[pymethods]
impl Queryable {
    fn recv(&self) -> PyResult<QueryableEvent> {
        self.inner
            .receiver()
            .recv()
            .map(|inner| QueryableEvent { inner })
            .map_err(to_py_err)
    }

    fn reply(&self, query_id: u64, key_expr: String, payload: Vec<u8>) -> PyResult<()> {
        self.rt
            .block_on(self.inner.reply(query_id, key_expr, payload))
            .map_err(to_py_err)
    }

    fn reply_err(&self, query_id: u64, payload: Vec<u8>) -> PyResult<()> {
        self.rt
            .block_on(self.inner.reply_err(query_id, payload))
            .map_err(to_py_err)
    }

    fn reply_delete(&self, query_id: u64, key_expr: String) -> PyResult<()> {
        self.rt
            .block_on(self.inner.reply_delete(query_id, key_expr))
            .map_err(to_py_err)
    }

    fn undeclare(&self) -> PyResult<()> {
        self.rt.block_on(self.inner.undeclare()).map_err(to_py_err)
    }
}

#[pymethods]
impl Querier {
    fn get(&self, py: Python<'_>, parameters: String) -> PyResult<PyObject> {
        let replies = self.rt.block_on(self.inner.get(parameters)).map_err(to_py_err)?;
        let values: Vec<String> = replies
            .drain()
            .filter_map(|reply| format_reply(&reply))
            .collect();
        Ok(values.into_py(py))
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
