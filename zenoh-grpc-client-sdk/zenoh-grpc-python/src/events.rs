use std::sync::Arc;

use pyo3::{
    exceptions::{PyRuntimeError, PyStopIteration},
    prelude::*,
};
use tokio::runtime::Runtime;
use zenoh_grpc_client_rs::{
    GrpcQuery as RsGrpcQuery, QueryStream as RsQueryStream, ReplyStream as RsReplyStream,
};
use zenoh_grpc_proto::v1 as pb;

use crate::common::to_py_err;

#[pyclass]
#[derive(Clone)]
pub(crate) struct SubscriberEvent {
    pub(crate) inner: pb::SubscriberEvent,
}

#[pyclass]
#[derive(Clone)]
pub(crate) struct Sample {
    pub(crate) inner: pb::Sample,
}

#[pyclass]
#[derive(Clone)]
pub(crate) struct ReplyError {
    pub(crate) inner: pb::ReplyError,
}

#[pyclass]
#[derive(Clone)]
pub(crate) struct Reply {
    pub(crate) inner: pb::Reply,
}

#[pyclass]
pub(crate) struct Query {
    rt: Arc<Runtime>,
    inner: Option<RsGrpcQuery>,
}

#[pyclass]
#[derive(Clone)]
pub(crate) struct QueryStream {
    rt: Arc<Runtime>,
    inner: RsQueryStream,
}

#[pyclass]
#[derive(Clone)]
pub(crate) struct ReplyStream {
    inner: RsReplyStream,
}

impl Sample {
    pub(crate) fn from_inner(inner: pb::Sample) -> Self {
        Self { inner }
    }
}

impl ReplyError {
    pub(crate) fn from_inner(inner: pb::ReplyError) -> Self {
        Self { inner }
    }
}

impl Reply {
    pub(crate) fn from_inner(inner: pb::Reply) -> Self {
        Self { inner }
    }
}

impl Query {
    pub(crate) fn from_inner(rt: Arc<Runtime>, inner: RsGrpcQuery) -> Self {
        Self {
            rt,
            inner: Some(inner),
        }
    }

    fn inner(&self) -> PyResult<&RsGrpcQuery> {
        self.inner
            .as_ref()
            .ok_or_else(|| PyRuntimeError::new_err("query already dropped"))
    }

    fn take_inner(&mut self) -> PyResult<RsGrpcQuery> {
        self.inner
            .take()
            .ok_or_else(|| PyRuntimeError::new_err("query already dropped"))
    }
}

impl QueryStream {
    pub(crate) fn from_inner(rt: Arc<Runtime>, inner: RsQueryStream) -> Self {
        Self { rt, inner }
    }

    fn next_query(&self) -> PyResult<Query> {
        match self.inner.recv() {
            Ok(inner) => Ok(Query::from_inner(self.rt.clone(), inner)),
            Err(err) if self.inner.is_closed() => Err(PyStopIteration::new_err(err.to_string())),
            Err(err) => Err(to_py_err(err)),
        }
    }
}

impl ReplyStream {
    pub(crate) fn from_inner(inner: RsReplyStream) -> Self {
        Self { inner }
    }

    fn next_reply(&self) -> PyResult<Reply> {
        match self.inner.recv() {
            Ok(inner) => Ok(Reply::from_inner(inner)),
            Err(err) if self.inner.is_closed() => Err(PyStopIteration::new_err(err.to_string())),
            Err(err) => Err(to_py_err(err)),
        }
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

    #[getter]
    fn encoding(&self) -> Option<String> {
        self.inner.sample.as_ref().map(|s| s.encoding.clone())
    }

    #[getter]
    fn timestamp(&self) -> Option<String> {
        self.inner.sample.as_ref().map(|s| s.timestamp.clone())
    }

    #[getter]
    fn kind(&self) -> Option<i32> {
        self.inner.sample.as_ref().map(|s| s.kind)
    }

    #[getter]
    fn attachment(&self) -> Option<Vec<u8>> {
        self.inner.sample.as_ref().map(|s| s.attachment.clone())
    }

    #[getter]
    fn source_info_id(&self) -> Option<String> {
        self.inner
            .sample
            .as_ref()
            .and_then(|s| s.source_info.as_ref().map(|info| info.id.clone()))
    }

    #[getter]
    fn source_info_sequence(&self) -> Option<u64> {
        self.inner
            .sample
            .as_ref()
            .and_then(|s| s.source_info.as_ref().map(|info| info.sequence))
    }
}

#[pymethods]
impl Sample {
    #[getter]
    fn key_expr(&self) -> String {
        self.inner.key_expr.clone()
    }

    #[getter]
    fn payload(&self) -> Vec<u8> {
        self.inner.payload.clone()
    }

    #[getter]
    fn encoding(&self) -> String {
        self.inner.encoding.clone()
    }

    #[getter]
    fn kind(&self) -> i32 {
        self.inner.kind
    }

    #[getter]
    fn attachment(&self) -> Vec<u8> {
        self.inner.attachment.clone()
    }

    #[getter]
    fn timestamp(&self) -> String {
        self.inner.timestamp.clone()
    }

    #[getter]
    fn source_info_id(&self) -> Option<String> {
        self.inner.source_info.as_ref().map(|info| info.id.clone())
    }

    #[getter]
    fn source_info_sequence(&self) -> Option<u64> {
        self.inner.source_info.as_ref().map(|info| info.sequence)
    }
}

#[pymethods]
impl ReplyError {
    #[getter]
    fn payload(&self) -> Vec<u8> {
        self.inner.payload.clone()
    }

    #[getter]
    fn encoding(&self) -> String {
        self.inner.encoding.clone()
    }
}

#[pymethods]
impl Reply {
    #[getter]
    fn ok(&self) -> bool {
        matches!(self.inner.result, Some(pb::reply::Result::Sample(_)))
    }

    #[getter]
    fn is_ok(&self) -> bool {
        matches!(self.inner.result, Some(pb::reply::Result::Sample(_)))
    }

    #[getter]
    fn sample(&self) -> Option<Sample> {
        match &self.inner.result {
            Some(pb::reply::Result::Sample(sample)) => Some(Sample::from_inner(sample.clone())),
            _ => None,
        }
    }

    #[getter]
    fn error(&self) -> Option<ReplyError> {
        match &self.inner.result {
            Some(pb::reply::Result::Error(error)) => Some(ReplyError::from_inner(error.clone())),
            _ => None,
        }
    }
}

#[pymethods]
impl Query {
    fn __enter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __exit__(
        mut slf: PyRefMut<'_, Self>,
        _exc_type: Option<&Bound<'_, PyAny>>,
        _exc: Option<&Bound<'_, PyAny>>,
        _tb: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        Query::drop(&mut slf)
    }

    #[getter]
    fn query_id(&self) -> PyResult<u64> {
        Ok(self.inner()?.query_id())
    }

    #[getter]
    fn selector(&self) -> PyResult<String> {
        Ok(self.inner()?.selector().to_string())
    }

    #[getter]
    fn key_expr(&self) -> PyResult<String> {
        Ok(self.inner()?.key_expr().to_string())
    }

    #[getter]
    fn parameters(&self) -> PyResult<String> {
        Ok(self.inner()?.parameters().to_string())
    }

    #[getter]
    fn payload(&self) -> PyResult<Vec<u8>> {
        Ok(self.inner()?.payload().to_vec())
    }

    #[getter]
    fn encoding(&self) -> PyResult<String> {
        Ok(self.inner()?.encoding().to_string())
    }

    #[getter]
    fn attachment(&self) -> PyResult<Vec<u8>> {
        Ok(self.inner()?.attachment().to_vec())
    }

    #[pyo3(signature = (key_expr, payload, encoding=None, attachment=None, timestamp=None))]
    fn reply(
        &self,
        key_expr: String,
        payload: Vec<u8>,
        encoding: Option<String>,
        attachment: Option<Vec<u8>>,
        timestamp: Option<String>,
    ) -> PyResult<()> {
        self.rt
            .block_on(self.inner()?.reply(
                key_expr,
                payload,
                encoding.unwrap_or_default(),
                attachment.unwrap_or_default(),
                timestamp.unwrap_or_default(),
            ))
            .map_err(to_py_err)
    }

    #[pyo3(signature = (payload, encoding=None))]
    fn reply_err(&self, payload: Vec<u8>, encoding: Option<String>) -> PyResult<()> {
        self.rt
            .block_on(
                self.inner()?
                    .reply_err(payload, encoding.unwrap_or_default()),
            )
            .map_err(to_py_err)
    }

    #[pyo3(signature = (key_expr, attachment=None, timestamp=None))]
    fn reply_delete(
        &self,
        key_expr: String,
        attachment: Option<Vec<u8>>,
        timestamp: Option<String>,
    ) -> PyResult<()> {
        self.rt
            .block_on(self.inner()?.reply_delete(
                key_expr,
                attachment.unwrap_or_default(),
                timestamp.unwrap_or_default(),
            ))
            .map_err(to_py_err)
    }

    fn drop(&mut self) -> PyResult<()> {
        let inner = self.take_inner()?;
        self.rt.block_on(inner.finish()).map_err(to_py_err)
    }
}

#[pymethods]
impl QueryStream {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(&self) -> PyResult<Query> {
        self.next_query()
    }

    fn recv(&self) -> PyResult<Query> {
        self.inner
            .recv()
            .map(|inner| Query::from_inner(self.rt.clone(), inner))
            .map_err(to_py_err)
    }

    fn try_recv(&self) -> PyResult<Option<Query>> {
        self.inner
            .try_recv()
            .map(|query| query.map(|inner| Query::from_inner(self.rt.clone(), inner)))
            .map_err(to_py_err)
    }

    fn dropped_count(&self) -> u64 {
        self.inner.dropped_count()
    }

    fn is_closed(&self) -> bool {
        self.inner.is_closed()
    }
}

#[pymethods]
impl ReplyStream {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(&self) -> PyResult<Reply> {
        self.next_reply()
    }

    fn recv(&self) -> PyResult<Reply> {
        self.inner.recv().map(Reply::from_inner).map_err(to_py_err)
    }

    fn try_recv(&self) -> PyResult<Option<Reply>> {
        self.inner
            .try_recv()
            .map(|reply| reply.map(Reply::from_inner))
            .map_err(to_py_err)
    }

    fn dropped_count(&self) -> u64 {
        self.inner.dropped_count()
    }

    fn is_closed(&self) -> bool {
        self.inner.is_closed()
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<SubscriberEvent>()?;
    module.add_class::<Sample>()?;
    module.add_class::<ReplyError>()?;
    module.add_class::<Reply>()?;
    module.add_class::<Query>()?;
    module.add_class::<QueryStream>()?;
    module.add_class::<ReplyStream>()?;
    Ok(())
}
