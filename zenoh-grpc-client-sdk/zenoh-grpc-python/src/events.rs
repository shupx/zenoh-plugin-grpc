use pyo3::prelude::*;
use zenoh_grpc_proto::v1 as pb;

#[pyclass]
#[derive(Clone)]
pub(crate) struct SubscriberEvent {
    pub(crate) inner: pb::SubscriberEvent,
}

#[pyclass]
#[derive(Clone)]
pub(crate) struct QueryableEvent {
    pub(crate) inner: pb::QueryableEvent,
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
    fn selector(&self) -> Option<String> {
        self.inner.query.as_ref().map(|q| q.selector.clone())
    }

    #[getter]
    fn parameters(&self) -> Option<String> {
        self.inner.query.as_ref().map(|q| q.parameters.clone())
    }

    #[getter]
    fn payload(&self) -> Option<Vec<u8>> {
        self.inner.query.as_ref().map(|q| q.payload.clone())
    }

    #[getter]
    fn encoding(&self) -> Option<String> {
        self.inner.query.as_ref().map(|q| q.encoding.clone())
    }

    #[getter]
    fn attachment(&self) -> Option<Vec<u8>> {
        self.inner.query.as_ref().map(|q| q.attachment.clone())
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<SubscriberEvent>()?;
    module.add_class::<QueryableEvent>()?;
    Ok(())
}
