mod common;
mod enums;
mod events;
mod objects;

use pyo3::prelude::*;

#[pymodule]
fn zenoh_grpc(_py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    objects::register(module)?;
    enums::register(module)?;
    events::register(module)?;
    Ok(())
}
