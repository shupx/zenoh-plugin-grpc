#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct zgrpc_session_t zgrpc_session_t;
typedef struct zgrpc_publisher_t zgrpc_publisher_t;
typedef struct zgrpc_subscriber_t zgrpc_subscriber_t;
typedef struct zgrpc_queryable_t zgrpc_queryable_t;
typedef struct zgrpc_querier_t zgrpc_querier_t;
typedef struct zgrpc_reply_stream_t zgrpc_reply_stream_t;

typedef enum zgrpc_sample_kind_t {
    ZGRPC_SAMPLE_KIND_UNSPECIFIED = 0,
    ZGRPC_SAMPLE_KIND_PUT = 1,
    ZGRPC_SAMPLE_KIND_DELETE = 2,
} zgrpc_sample_kind_t;

typedef enum zgrpc_congestion_control_t {
    ZGRPC_CONGESTION_CONTROL_UNSPECIFIED = 0,
    ZGRPC_CONGESTION_CONTROL_BLOCK = 1,
    ZGRPC_CONGESTION_CONTROL_DROP = 2,
} zgrpc_congestion_control_t;

typedef enum zgrpc_priority_t {
    ZGRPC_PRIORITY_UNSPECIFIED = 0,
    ZGRPC_PRIORITY_REAL_TIME = 1,
    ZGRPC_PRIORITY_INTERACTIVE_HIGH = 2,
    ZGRPC_PRIORITY_INTERACTIVE_LOW = 3,
    ZGRPC_PRIORITY_DATA_HIGH = 4,
    ZGRPC_PRIORITY_DATA = 5,
    ZGRPC_PRIORITY_DATA_LOW = 6,
    ZGRPC_PRIORITY_BACKGROUND = 7,
} zgrpc_priority_t;

typedef enum zgrpc_reliability_t {
    ZGRPC_RELIABILITY_UNSPECIFIED = 0,
    ZGRPC_RELIABILITY_BEST_EFFORT = 1,
    ZGRPC_RELIABILITY_RELIABLE = 2,
} zgrpc_reliability_t;

typedef enum zgrpc_locality_t {
    ZGRPC_LOCALITY_UNSPECIFIED = 0,
    ZGRPC_LOCALITY_ANY = 1,
    ZGRPC_LOCALITY_SESSION_LOCAL = 2,
    ZGRPC_LOCALITY_REMOTE = 3,
} zgrpc_locality_t;

typedef enum zgrpc_query_target_t {
    ZGRPC_QUERY_TARGET_UNSPECIFIED = 0,
    ZGRPC_QUERY_TARGET_BEST_MATCHING = 1,
    ZGRPC_QUERY_TARGET_ALL = 2,
    ZGRPC_QUERY_TARGET_ALL_COMPLETE = 3,
} zgrpc_query_target_t;

typedef enum zgrpc_consolidation_mode_t {
    ZGRPC_CONSOLIDATION_MODE_UNSPECIFIED = 0,
    ZGRPC_CONSOLIDATION_MODE_AUTO = 1,
    ZGRPC_CONSOLIDATION_MODE_NONE = 2,
    ZGRPC_CONSOLIDATION_MODE_MONOTONIC = 3,
    ZGRPC_CONSOLIDATION_MODE_LATEST = 4,
} zgrpc_consolidation_mode_t;

typedef struct zgrpc_sample_t {
    char* key_expr;
    uint8_t* payload;
    size_t payload_len;
} zgrpc_sample_t;

typedef struct zgrpc_query_t {
    uint64_t query_id;
    char* key_expr;
    char* parameters;
    uint8_t* payload;
    size_t payload_len;
} zgrpc_query_t;

typedef struct zgrpc_reply_t {
    int is_error;
    char* key_expr;
    uint8_t* payload;
    size_t payload_len;
} zgrpc_reply_t;

typedef struct zgrpc_source_info_t {
    int is_valid;
    char* id;
    uint64_t sequence;
} zgrpc_source_info_t;

typedef struct zgrpc_sample_ex_t {
    char* key_expr;
    uint8_t* payload;
    size_t payload_len;
    char* encoding;
    int kind;
    uint8_t* attachment;
    size_t attachment_len;
    char* timestamp;
    zgrpc_source_info_t source_info;
} zgrpc_sample_ex_t;

typedef struct zgrpc_subscriber_event_t {
    int has_sample;
    zgrpc_sample_ex_t sample;
} zgrpc_subscriber_event_t;

typedef struct zgrpc_query_ex_t {
    uint64_t query_id;
    char* selector;
    char* key_expr;
    char* parameters;
    uint8_t* payload;
    size_t payload_len;
    char* encoding;
    uint8_t* attachment;
    size_t attachment_len;
} zgrpc_query_ex_t;

typedef struct zgrpc_reply_error_t {
    uint8_t* payload;
    size_t payload_len;
    char* encoding;
} zgrpc_reply_error_t;

typedef struct zgrpc_reply_ex_t {
    int has_sample;
    zgrpc_sample_ex_t sample;
    int has_error;
    zgrpc_reply_error_t error;
} zgrpc_reply_ex_t;

typedef struct zgrpc_session_put_options_t {
    const char* encoding;
    int congestion_control;
    int priority;
    bool express;
    const uint8_t* attachment;
    size_t attachment_len;
    const char* timestamp;
    int allowed_destination;
} zgrpc_session_put_options_t;

typedef struct zgrpc_session_delete_options_t {
    int congestion_control;
    int priority;
    bool express;
    const uint8_t* attachment;
    size_t attachment_len;
    const char* timestamp;
    int allowed_destination;
} zgrpc_session_delete_options_t;

typedef struct zgrpc_session_get_options_t {
    int target;
    int consolidation;
    uint64_t timeout_ms;
    const uint8_t* payload;
    size_t payload_len;
    const char* encoding;
    const uint8_t* attachment;
    size_t attachment_len;
    int allowed_destination;
} zgrpc_session_get_options_t;

typedef struct zgrpc_publisher_options_t {
    const char* encoding;
    int congestion_control;
    int priority;
    bool express;
    int reliability;
    int allowed_destination;
} zgrpc_publisher_options_t;

typedef struct zgrpc_subscriber_options_t {
    int allowed_origin;
} zgrpc_subscriber_options_t;

typedef struct zgrpc_queryable_options_t {
    bool complete;
    int allowed_origin;
} zgrpc_queryable_options_t;

typedef struct zgrpc_querier_options_t {
    int target;
    int consolidation;
    uint64_t timeout_ms;
    int allowed_destination;
} zgrpc_querier_options_t;

typedef struct zgrpc_querier_get_options_t {
    const uint8_t* payload;
    size_t payload_len;
    const char* encoding;
    const uint8_t* attachment;
    size_t attachment_len;
} zgrpc_querier_get_options_t;

typedef struct zgrpc_publisher_put_options_t {
    const char* encoding;
    const uint8_t* attachment;
    size_t attachment_len;
    const char* timestamp;
} zgrpc_publisher_put_options_t;

typedef struct zgrpc_publisher_delete_options_t {
    const uint8_t* attachment;
    size_t attachment_len;
    const char* timestamp;
} zgrpc_publisher_delete_options_t;

typedef struct zgrpc_query_reply_options_t {
    const char* encoding;
    const uint8_t* attachment;
    size_t attachment_len;
    const char* timestamp;
} zgrpc_query_reply_options_t;

typedef struct zgrpc_query_reply_err_options_t {
    const char* encoding;
} zgrpc_query_reply_err_options_t;

typedef struct zgrpc_query_reply_delete_options_t {
    const uint8_t* attachment;
    size_t attachment_len;
    const char* timestamp;
} zgrpc_query_reply_delete_options_t;

typedef void (*zgrpc_drop_callback_t)(void* context);
typedef void (*zgrpc_subscriber_callback_t)(const zgrpc_subscriber_event_t* event, void* context);
typedef void (*zgrpc_queryable_callback_t)(
    zgrpc_queryable_t* queryable,
    const zgrpc_query_ex_t* query,
    void* context
);

zgrpc_session_t* zgrpc_session_connect(const char* endpoint);
zgrpc_session_t* zgrpc_open_tcp(const char* addr);
zgrpc_session_t* zgrpc_open_unix(const char* path);
char* zgrpc_session_info(zgrpc_session_t* session);
void zgrpc_close(zgrpc_session_t* session);

void zgrpc_session_put_options_default(zgrpc_session_put_options_t* options);
void zgrpc_session_delete_options_default(zgrpc_session_delete_options_t* options);
void zgrpc_session_get_options_default(zgrpc_session_get_options_t* options);
void zgrpc_publisher_options_default(zgrpc_publisher_options_t* options);
void zgrpc_subscriber_options_default(zgrpc_subscriber_options_t* options);
void zgrpc_queryable_options_default(zgrpc_queryable_options_t* options);
void zgrpc_querier_options_default(zgrpc_querier_options_t* options);
void zgrpc_querier_get_options_default(zgrpc_querier_get_options_t* options);
void zgrpc_publisher_put_options_default(zgrpc_publisher_put_options_t* options);
void zgrpc_publisher_delete_options_default(zgrpc_publisher_delete_options_t* options);
void zgrpc_query_reply_options_default(zgrpc_query_reply_options_t* options);
void zgrpc_query_reply_err_options_default(zgrpc_query_reply_err_options_t* options);
void zgrpc_query_reply_delete_options_default(zgrpc_query_reply_delete_options_t* options);

int zgrpc_session_put(
    zgrpc_session_t* session,
    const char* key_expr,
    const uint8_t* payload,
    size_t payload_len
);
int zgrpc_session_put_with_options(
    zgrpc_session_t* session,
    const char* key_expr,
    const uint8_t* payload,
    size_t payload_len,
    const zgrpc_session_put_options_t* options
);
int zgrpc_session_delete(zgrpc_session_t* session, const char* key_expr);
int zgrpc_session_delete_with_options(
    zgrpc_session_t* session,
    const char* key_expr,
    const zgrpc_session_delete_options_t* options
);
zgrpc_reply_stream_t* zgrpc_session_get(
    zgrpc_session_t* session,
    const char* selector,
    const zgrpc_session_get_options_t* options
);

zgrpc_publisher_t* zgrpc_declare_publisher(zgrpc_session_t* session, const char* key_expr);
zgrpc_publisher_t* zgrpc_declare_publisher_with_options(
    zgrpc_session_t* session,
    const char* key_expr,
    const zgrpc_publisher_options_t* options
);
int zgrpc_publisher_put(zgrpc_publisher_t* publisher, const uint8_t* payload, size_t payload_len);
int zgrpc_publisher_put_with_options(
    zgrpc_publisher_t* publisher,
    const uint8_t* payload,
    size_t payload_len,
    const zgrpc_publisher_put_options_t* options
);
int zgrpc_publisher_delete(zgrpc_publisher_t* publisher);
int zgrpc_publisher_delete_with_options(
    zgrpc_publisher_t* publisher,
    const zgrpc_publisher_delete_options_t* options
);
uint64_t zgrpc_publisher_send_dropped_count(zgrpc_publisher_t* publisher);
int zgrpc_publisher_undeclare(zgrpc_publisher_t* publisher);

zgrpc_subscriber_t* zgrpc_declare_subscriber(zgrpc_session_t* session, const char* key_expr);
zgrpc_subscriber_t* zgrpc_declare_subscriber_with_options(
    zgrpc_session_t* session,
    const char* key_expr,
    const zgrpc_subscriber_options_t* options
);
zgrpc_subscriber_t* zgrpc_declare_subscriber_with_callback(
    zgrpc_session_t* session,
    const char* key_expr,
    zgrpc_subscriber_callback_t callback,
    void* context,
    zgrpc_drop_callback_t on_drop,
    const zgrpc_subscriber_options_t* options
);
int zgrpc_subscriber_recv(zgrpc_subscriber_t* subscriber, zgrpc_sample_t* sample_out);
int zgrpc_subscriber_try_recv(zgrpc_subscriber_t* subscriber, zgrpc_sample_t* sample_out);
int zgrpc_subscriber_recv_event(
    zgrpc_subscriber_t* subscriber,
    zgrpc_subscriber_event_t* event_out
);
int zgrpc_subscriber_try_recv_event(
    zgrpc_subscriber_t* subscriber,
    zgrpc_subscriber_event_t* event_out
);
uint64_t zgrpc_subscriber_dropped_count(zgrpc_subscriber_t* subscriber);
int zgrpc_subscriber_undeclare(zgrpc_subscriber_t* subscriber);

zgrpc_queryable_t* zgrpc_declare_queryable(zgrpc_session_t* session, const char* key_expr);
zgrpc_queryable_t* zgrpc_declare_queryable_with_options(
    zgrpc_session_t* session,
    const char* key_expr,
    const zgrpc_queryable_options_t* options
);
zgrpc_queryable_t* zgrpc_declare_queryable_with_callback(
    zgrpc_session_t* session,
    const char* key_expr,
    zgrpc_queryable_callback_t callback,
    void* context,
    zgrpc_drop_callback_t on_drop,
    const zgrpc_queryable_options_t* options
);
int zgrpc_queryable_recv(zgrpc_queryable_t* queryable, zgrpc_query_t* query_out);
int zgrpc_queryable_try_recv(zgrpc_queryable_t* queryable, zgrpc_query_t* query_out);
int zgrpc_queryable_recv_ex(zgrpc_queryable_t* queryable, zgrpc_query_ex_t* query_out);
int zgrpc_queryable_try_recv_ex(zgrpc_queryable_t* queryable, zgrpc_query_ex_t* query_out);
int zgrpc_queryable_reply(
    zgrpc_queryable_t* queryable,
    uint64_t query_id,
    const char* key_expr,
    const uint8_t* payload,
    size_t payload_len
);
int zgrpc_queryable_reply_with_options(
    zgrpc_queryable_t* queryable,
    uint64_t query_id,
    const char* key_expr,
    const uint8_t* payload,
    size_t payload_len,
    const zgrpc_query_reply_options_t* options
);
int zgrpc_queryable_reply_err(
    zgrpc_queryable_t* queryable,
    uint64_t query_id,
    const uint8_t* payload,
    size_t payload_len
);
int zgrpc_queryable_reply_err_with_options(
    zgrpc_queryable_t* queryable,
    uint64_t query_id,
    const uint8_t* payload,
    size_t payload_len,
    const zgrpc_query_reply_err_options_t* options
);
int zgrpc_queryable_reply_delete(
    zgrpc_queryable_t* queryable,
    uint64_t query_id,
    const char* key_expr
);
int zgrpc_queryable_reply_delete_with_options(
    zgrpc_queryable_t* queryable,
    uint64_t query_id,
    const char* key_expr,
    const zgrpc_query_reply_delete_options_t* options
);
int zgrpc_queryable_finish(zgrpc_queryable_t* queryable, uint64_t query_id);
int zgrpc_queryable_dropped_count(zgrpc_queryable_t* queryable, uint64_t* count_out);
int zgrpc_queryable_is_closed(zgrpc_queryable_t* queryable, int* is_closed_out);
uint64_t zgrpc_queryable_send_dropped_count(zgrpc_queryable_t* queryable);
int zgrpc_queryable_undeclare(zgrpc_queryable_t* queryable);

zgrpc_querier_t* zgrpc_declare_querier(zgrpc_session_t* session, const char* key_expr);
zgrpc_querier_t* zgrpc_declare_querier_with_options(
    zgrpc_session_t* session,
    const char* key_expr,
    const zgrpc_querier_options_t* options
);
int zgrpc_querier_get(
    zgrpc_querier_t* querier,
    const char* parameters,
    zgrpc_reply_t* reply_out
);
int zgrpc_querier_get_with_options(
    zgrpc_querier_t* querier,
    const char* parameters,
    const zgrpc_querier_get_options_t* options,
    zgrpc_reply_t* reply_out
);
zgrpc_reply_stream_t* zgrpc_querier_get_stream(
    zgrpc_querier_t* querier,
    const char* parameters,
    const zgrpc_querier_get_options_t* options
);
int zgrpc_querier_undeclare(zgrpc_querier_t* querier);

int zgrpc_reply_stream_recv(zgrpc_reply_stream_t* stream, zgrpc_reply_ex_t* reply_out);
int zgrpc_reply_stream_try_recv(zgrpc_reply_stream_t* stream, zgrpc_reply_ex_t* reply_out);
uint64_t zgrpc_reply_stream_dropped_count(zgrpc_reply_stream_t* stream);
int zgrpc_reply_stream_is_closed(zgrpc_reply_stream_t* stream);
void zgrpc_reply_stream_drop(zgrpc_reply_stream_t* stream);

void zgrpc_string_free(char* value);
void zgrpc_bytes_free(uint8_t* value, size_t len);
void zgrpc_sample_free(zgrpc_sample_t* sample);
void zgrpc_query_free(zgrpc_query_t* query);
void zgrpc_reply_free(zgrpc_reply_t* reply);
void zgrpc_source_info_free(zgrpc_source_info_t* source_info);
void zgrpc_sample_ex_free(zgrpc_sample_ex_t* sample);
void zgrpc_subscriber_event_free(zgrpc_subscriber_event_t* event);
void zgrpc_query_ex_free(zgrpc_query_ex_t* query);
void zgrpc_reply_error_free(zgrpc_reply_error_t* error);
void zgrpc_reply_ex_free(zgrpc_reply_ex_t* reply);

#ifdef __cplusplus
}
#endif
