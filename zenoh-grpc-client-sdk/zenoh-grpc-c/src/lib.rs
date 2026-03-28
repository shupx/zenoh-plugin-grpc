use std::{ffi::CStr, os::raw::c_char, path::PathBuf, sync::Arc};

use tokio::runtime::Runtime;
use zenoh_grpc_client_rs::{ConnectAddr, GrpcSession};

fn runtime() -> Arc<Runtime> {
    static RT: std::sync::OnceLock<Arc<Runtime>> = std::sync::OnceLock::new();
    RT.get_or_init(|| Arc::new(Runtime::new().expect("tokio runtime"))).clone()
}

#[repr(C)]
pub struct zgrpc_session_t {
    rt: Arc<Runtime>,
    inner: GrpcSession,
}

unsafe fn cstr(ptr: *const c_char) -> String {
    CStr::from_ptr(ptr).to_string_lossy().into_owned()
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
    match rt.block_on(GrpcSession::connect(ConnectAddr::Unix(PathBuf::from(cstr(path))))) {
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
