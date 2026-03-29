use std::{path::PathBuf, sync::Arc};

use pyo3::{exceptions::PyRuntimeError, prelude::*};
use tokio::runtime::{Builder, Runtime};
use zenoh_grpc_client_rs::ConnectAddr;

pub(crate) fn runtime() -> Arc<Runtime> {
    static RT: std::sync::OnceLock<Arc<Runtime>> = std::sync::OnceLock::new();
    // RT.get_or_init(|| Arc::new(Runtime::new().expect("tokio runtime")))
    //     .clone()
    RT.get_or_init(|| {
        let rt = Builder::new_multi_thread()
            .worker_threads(2) // too many threads are not helpful for IO operations, spawn() will use these threads.
            .max_blocking_threads(16) // allow more threads for cpu heavy operations, spawn_blocking() will use these threads.
            .enable_all()
            .build()
            .expect("tokio runtime");

        Arc::new(rt)
    })
    .clone()
}

pub(crate) fn to_py_err<E: std::fmt::Display>(err: E) -> PyErr {
    PyRuntimeError::new_err(err.to_string())
}

pub(crate) fn parse_connect_addr(endpoint: &str) -> PyResult<ConnectAddr> {
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
