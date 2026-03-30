from typing import Callable, ClassVar, Iterator, Optional


class SampleKind:
    UNSPECIFIED: ClassVar[int]
    PUT: ClassVar[int]
    DELETE: ClassVar[int]


class CongestionControl:
    UNSPECIFIED: ClassVar[int]
    BLOCK: ClassVar[int]
    DROP: ClassVar[int]


class Priority:
    UNSPECIFIED: ClassVar[int]
    REAL_TIME: ClassVar[int]
    INTERACTIVE_HIGH: ClassVar[int]
    INTERACTIVE_LOW: ClassVar[int]
    DATA_HIGH: ClassVar[int]
    DATA: ClassVar[int]
    DATA_LOW: ClassVar[int]
    BACKGROUND: ClassVar[int]


class Reliability:
    UNSPECIFIED: ClassVar[int]
    BEST_EFFORT: ClassVar[int]
    RELIABLE: ClassVar[int]


class Locality:
    UNSPECIFIED: ClassVar[int]
    ANY: ClassVar[int]
    SESSION_LOCAL: ClassVar[int]
    REMOTE: ClassVar[int]


class QueryTarget:
    UNSPECIFIED: ClassVar[int]
    BEST_MATCHING: ClassVar[int]
    ALL: ClassVar[int]
    ALL_COMPLETE: ClassVar[int]


class ConsolidationMode:
    UNSPECIFIED: ClassVar[int]
    AUTO: ClassVar[int]
    NONE: ClassVar[int]
    MONOTONIC: ClassVar[int]
    LATEST: ClassVar[int]


class SubscriberEvent:
    @property
    def key_expr(self) -> Optional[str]: ...
    @property
    def payload(self) -> Optional[bytes]: ...
    @property
    def encoding(self) -> Optional[str]: ...
    @property
    def timestamp(self) -> Optional[str]: ...
    @property
    def kind(self) -> Optional[int]: ...
    @property
    def attachment(self) -> Optional[bytes]: ...
    @property
    def source_info_id(self) -> Optional[str]: ...
    @property
    def source_info_sequence(self) -> Optional[int]: ...


class Sample:
    @property
    def key_expr(self) -> str: ...
    @property
    def payload(self) -> bytes: ...
    @property
    def encoding(self) -> str: ...
    @property
    def kind(self) -> int: ...
    @property
    def attachment(self) -> bytes: ...
    @property
    def timestamp(self) -> str: ...
    @property
    def source_info_id(self) -> Optional[str]: ...
    @property
    def source_info_sequence(self) -> Optional[int]: ...


class ReplyError:
    @property
    def payload(self) -> bytes: ...
    @property
    def encoding(self) -> str: ...


class Reply:
    @property
    def ok(self) -> bool: ...
    @property
    def is_ok(self) -> bool: ...
    @property
    def sample(self) -> Optional[Sample]: ...
    @property
    def error(self) -> Optional[ReplyError]: ...


class Query:
    def __enter__(self) -> "Query": ...
    def __exit__(self, exc_type: object, exc: object, tb: object) -> None: ...
    @property
    def query_id(self) -> int: ...
    @property
    def selector(self) -> str: ...
    @property
    def key_expr(self) -> str: ...
    @property
    def parameters(self) -> str: ...
    @property
    def payload(self) -> bytes: ...
    @property
    def encoding(self) -> str: ...
    @property
    def attachment(self) -> bytes: ...
    def reply(
        self,
        key_expr: str,
        payload: bytes,
        encoding: Optional[str] = ...,
        attachment: Optional[bytes] = ...,
        timestamp: Optional[str] = ...,
    ) -> None: ...
    def reply_err(self, payload: bytes, encoding: Optional[str] = ...) -> None: ...
    def reply_delete(
        self,
        key_expr: str,
        attachment: Optional[bytes] = ...,
        timestamp: Optional[str] = ...,
    ) -> None: ...
    def drop(self) -> None: ...


class ReplyStream(Iterator[Reply]):
    def __iter__(self) -> "ReplyStream": ...
    def __next__(self) -> Reply: ...
    def recv(self) -> Reply: ...
    def try_recv(self) -> Optional[Reply]: ...
    def dropped_count(self) -> int: ...
    def is_closed(self) -> bool: ...


class Session:
    @staticmethod
    def connect(endpoint: str = "unix:///tmp/zenoh-grpc.sock") -> "Session": ...
    def __enter__(self) -> "Session": ...
    def __exit__(self, exc_type: object, exc: object, tb: object) -> None: ...
    def info(self) -> str: ...
    def close(self) -> None: ...
    def put(
        self,
        key_expr: str,
        payload: bytes,
        encoding: Optional[str] = ...,
        congestion_control: Optional[int] = ...,
        priority: Optional[int] = ...,
        express: bool = ...,
        attachment: Optional[bytes] = ...,
        timestamp: Optional[str] = ...,
        allowed_destination: Optional[int] = ...,
    ) -> None: ...
    def delete(
        self,
        key_expr: str,
        congestion_control: Optional[int] = ...,
        priority: Optional[int] = ...,
        express: bool = ...,
        attachment: Optional[bytes] = ...,
        timestamp: Optional[str] = ...,
        allowed_destination: Optional[int] = ...,
    ) -> None: ...
    def get(
        self,
        selector: str,
        target: Optional[int] = ...,
        consolidation: Optional[int] = ...,
        timeout_ms: Optional[int] = ...,
        payload: Optional[bytes] = ...,
        encoding: Optional[str] = ...,
        attachment: Optional[bytes] = ...,
        allowed_destination: Optional[int] = ...,
    ) -> ReplyStream: ...
    def declare_publisher(
        self,
        key_expr: str,
        encoding: Optional[str] = ...,
        congestion_control: Optional[int] = ...,
        priority: Optional[int] = ...,
        express: bool = ...,
        reliability: Optional[int] = ...,
        allowed_destination: Optional[int] = ...,
    ) -> "Publisher": ...
    def declare_subscriber(
        self,
        key_expr: str,
        callback: Optional[Callable[[SubscriberEvent], object]] = ...,
        allowed_origin: Optional[int] = ...,
    ) -> "Subscriber": ...
    def declare_queryable(
        self,
        key_expr: str,
        callback: Optional[Callable[[Query], object]] = ...,
        complete: bool = ...,
        allowed_origin: Optional[int] = ...,
    ) -> "Queryable": ...
    def declare_querier(
        self,
        key_expr: str,
        target: Optional[int] = ...,
        consolidation: Optional[int] = ...,
        timeout_ms: Optional[int] = ...,
        allowed_destination: Optional[int] = ...,
    ) -> "Querier": ...


class Publisher:
    def __enter__(self) -> "Publisher": ...
    def __exit__(self, exc_type: object, exc: object, tb: object) -> None: ...
    def put(
        self,
        payload: bytes,
        encoding: Optional[str] = ...,
        attachment: Optional[bytes] = ...,
        timestamp: Optional[str] = ...,
    ) -> None: ...
    def delete(
        self,
        attachment: Optional[bytes] = ...,
        timestamp: Optional[str] = ...,
    ) -> None: ...
    def undeclare(self) -> None: ...
    def send_dropped_count(self) -> int: ...


class Subscriber:
    def __enter__(self) -> "Subscriber": ...
    def __exit__(self, exc_type: object, exc: object, tb: object) -> None: ...
    def recv(self) -> SubscriberEvent: ...
    def try_recv(self) -> Optional[SubscriberEvent]: ...
    def undeclare(self) -> None: ...
    def dropped_count(self) -> int: ...


class Queryable(Iterator[Query]):
    def __enter__(self) -> "Queryable": ...
    def __exit__(self, exc_type: object, exc: object, tb: object) -> None: ...
    def __iter__(self) -> "Queryable": ...
    def __next__(self) -> Query: ...
    def recv(self) -> Query: ...
    def try_recv(self) -> Optional[Query]: ...
    def dropped_count(self) -> int: ...
    def is_closed(self) -> bool: ...
    def undeclare(self) -> None: ...
    def send_dropped_count(self) -> int: ...


class Querier:
    def __enter__(self) -> "Querier": ...
    def __exit__(self, exc_type: object, exc: object, tb: object) -> None: ...
    def get(
        self,
        parameters: Optional[str] = ...,
        payload: Optional[bytes] = ...,
        encoding: Optional[str] = ...,
        attachment: Optional[bytes] = ...,
    ) -> ReplyStream: ...
    def undeclare(self) -> None: ...

