use pyo3::prelude::*;
use zenoh_grpc_proto::v1 as pb;

#[pyclass]
pub(crate) struct SampleKind;

#[pyclass]
pub(crate) struct CongestionControl;

#[pyclass]
pub(crate) struct Priority;

#[pyclass]
pub(crate) struct Reliability;

#[pyclass]
pub(crate) struct Locality;

#[pyclass]
pub(crate) struct QueryTarget;

#[pyclass]
pub(crate) struct ConsolidationMode;

macro_rules! impl_enum_classattrs {
    ($name:ident, { $($attr:ident => $value:expr),* $(,)? }) => {
        #[pymethods]
        impl $name {
            $(
                #[classattr]
                const $attr: i32 = $value as i32;
            )*
        }
    };
}

impl_enum_classattrs!(SampleKind, {
    UNSPECIFIED => pb::SampleKind::Unspecified,
    PUT => pb::SampleKind::Put,
    DELETE => pb::SampleKind::Delete,
});

impl_enum_classattrs!(CongestionControl, {
    UNSPECIFIED => pb::CongestionControl::Unspecified,
    BLOCK => pb::CongestionControl::Block,
    DROP => pb::CongestionControl::Drop,
});

impl_enum_classattrs!(Priority, {
    UNSPECIFIED => pb::Priority::Unspecified,
    REAL_TIME => pb::Priority::RealTime,
    INTERACTIVE_HIGH => pb::Priority::InteractiveHigh,
    INTERACTIVE_LOW => pb::Priority::InteractiveLow,
    DATA_HIGH => pb::Priority::DataHigh,
    DATA => pb::Priority::Data,
    DATA_LOW => pb::Priority::DataLow,
    BACKGROUND => pb::Priority::Background,
});

impl_enum_classattrs!(Reliability, {
    UNSPECIFIED => pb::Reliability::Unspecified,
    BEST_EFFORT => pb::Reliability::BestEffort,
    RELIABLE => pb::Reliability::Reliable,
});

impl_enum_classattrs!(Locality, {
    UNSPECIFIED => pb::Locality::Unspecified,
    ANY => pb::Locality::Any,
    SESSION_LOCAL => pb::Locality::SessionLocal,
    REMOTE => pb::Locality::Remote,
});

impl_enum_classattrs!(QueryTarget, {
    UNSPECIFIED => pb::QueryTarget::Unspecified,
    BEST_MATCHING => pb::QueryTarget::BestMatching,
    ALL => pb::QueryTarget::All,
    ALL_COMPLETE => pb::QueryTarget::AllComplete,
});

impl_enum_classattrs!(ConsolidationMode, {
    UNSPECIFIED => pb::ConsolidationMode::Unspecified,
    AUTO => pb::ConsolidationMode::Auto,
    NONE => pb::ConsolidationMode::None,
    MONOTONIC => pb::ConsolidationMode::Monotonic,
    LATEST => pb::ConsolidationMode::Latest,
});

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<SampleKind>()?;
    module.add_class::<CongestionControl>()?;
    module.add_class::<Priority>()?;
    module.add_class::<Reliability>()?;
    module.add_class::<Locality>()?;
    module.add_class::<QueryTarget>()?;
    module.add_class::<ConsolidationMode>()?;
    Ok(())
}
