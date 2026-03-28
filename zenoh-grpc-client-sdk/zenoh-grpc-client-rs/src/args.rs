#[derive(Debug, Clone, Default)]
pub struct SessionPutArgs {
    pub key_expr: String,
    pub payload: Vec<u8>,
    pub encoding: String,
    pub congestion_control: i32,
    pub priority: i32,
    pub express: bool,
    pub attachment: Vec<u8>,
    pub timestamp: String,
    pub allowed_destination: i32,
}

#[derive(Debug, Clone, Default)]
pub struct SessionDeleteArgs {
    pub key_expr: String,
    pub congestion_control: i32,
    pub priority: i32,
    pub express: bool,
    pub attachment: Vec<u8>,
    pub timestamp: String,
    pub allowed_destination: i32,
}

#[derive(Debug, Clone)]
pub struct SessionGetArgs {
    pub selector: String,
    pub target: i32,
    pub consolidation: i32,
    pub timeout_ms: u64,
    pub payload: Vec<u8>,
    pub encoding: String,
    pub attachment: Vec<u8>,
    pub allowed_destination: i32,
}

impl Default for SessionGetArgs {
    fn default() -> Self {
        Self {
            selector: String::new(),
            target: 0,
            consolidation: 0,
            timeout_ms: 0,
            payload: Vec::new(),
            encoding: String::new(),
            attachment: Vec::new(),
            allowed_destination: 0,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct DeclarePublisherArgs {
    pub key_expr: String,
    pub encoding: String,
    pub congestion_control: i32,
    pub priority: i32,
    pub express: bool,
    pub reliability: i32,
    pub allowed_destination: i32,
}

#[derive(Debug, Clone, Default)]
pub struct DeclareSubscriberArgs {
    pub key_expr: String,
    pub allowed_origin: i32,
}

#[derive(Debug, Clone, Default)]
pub struct DeclareQueryableArgs {
    pub key_expr: String,
    pub complete: bool,
    pub allowed_origin: i32,
}

#[derive(Debug, Clone)]
pub struct DeclareQuerierArgs {
    pub key_expr: String,
    pub target: i32,
    pub consolidation: i32,
    pub timeout_ms: u64,
    pub allowed_destination: i32,
}

impl Default for DeclareQuerierArgs {
    fn default() -> Self {
        Self {
            key_expr: String::new(),
            target: 0,
            consolidation: 0,
            timeout_ms: 0,
            allowed_destination: 0,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct PublisherPutArgs {
    pub payload: Vec<u8>,
    pub encoding: String,
    pub attachment: Vec<u8>,
    pub timestamp: String,
}

#[derive(Debug, Clone, Default)]
pub struct PublisherDeleteArgs {
    pub attachment: Vec<u8>,
    pub timestamp: String,
}

#[derive(Debug, Clone, Default)]
pub struct QueryReplyArgs {
    pub query_id: u64,
    pub key_expr: String,
    pub payload: Vec<u8>,
    pub encoding: String,
    pub attachment: Vec<u8>,
    pub timestamp: String,
}

#[derive(Debug, Clone, Default)]
pub struct QueryReplyErrArgs {
    pub query_id: u64,
    pub payload: Vec<u8>,
    pub encoding: String,
}

#[derive(Debug, Clone, Default)]
pub struct QueryReplyDeleteArgs {
    pub query_id: u64,
    pub key_expr: String,
    pub attachment: Vec<u8>,
    pub timestamp: String,
}

#[derive(Debug, Clone, Default)]
pub struct QuerierGetArgs {
    pub parameters: String,
    pub payload: Vec<u8>,
    pub encoding: String,
    pub attachment: Vec<u8>,
}
