#pragma once

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

zgrpc_session_t* zgrpc_open_tcp(const char* addr);
zgrpc_session_t* zgrpc_open_unix(const char* path);
void zgrpc_close(zgrpc_session_t* session);

int zgrpc_session_put(zgrpc_session_t* session, const char* key_expr, const uint8_t* payload, size_t payload_len);
int zgrpc_session_delete(zgrpc_session_t* session, const char* key_expr);

zgrpc_publisher_t* zgrpc_declare_publisher(zgrpc_session_t* session, const char* key_expr);
int zgrpc_publisher_put(zgrpc_publisher_t* publisher, const uint8_t* payload, size_t payload_len);
int zgrpc_publisher_delete(zgrpc_publisher_t* publisher);
int zgrpc_publisher_undeclare(zgrpc_publisher_t* publisher);

zgrpc_subscriber_t* zgrpc_declare_subscriber(zgrpc_session_t* session, const char* key_expr);
int zgrpc_subscriber_recv(zgrpc_subscriber_t* subscriber, zgrpc_sample_t* sample_out);
int zgrpc_subscriber_try_recv(zgrpc_subscriber_t* subscriber, zgrpc_sample_t* sample_out);
int zgrpc_subscriber_undeclare(zgrpc_subscriber_t* subscriber);

zgrpc_queryable_t* zgrpc_declare_queryable(zgrpc_session_t* session, const char* key_expr);
int zgrpc_queryable_recv(zgrpc_queryable_t* queryable, zgrpc_query_t* query_out);
int zgrpc_queryable_reply(zgrpc_queryable_t* queryable, uint64_t query_id, const char* key_expr, const uint8_t* payload, size_t payload_len);
int zgrpc_queryable_reply_err(zgrpc_queryable_t* queryable, uint64_t query_id, const uint8_t* payload, size_t payload_len);
int zgrpc_queryable_reply_delete(zgrpc_queryable_t* queryable, uint64_t query_id, const char* key_expr);
int zgrpc_queryable_undeclare(zgrpc_queryable_t* queryable);

zgrpc_querier_t* zgrpc_declare_querier(zgrpc_session_t* session, const char* key_expr);
int zgrpc_querier_get(zgrpc_querier_t* querier, const char* parameters, zgrpc_reply_t* reply_out);
int zgrpc_querier_undeclare(zgrpc_querier_t* querier);

void zgrpc_string_free(char* value);
void zgrpc_bytes_free(uint8_t* value, size_t len);
void zgrpc_sample_free(zgrpc_sample_t* sample);
void zgrpc_query_free(zgrpc_query_t* query);
void zgrpc_reply_free(zgrpc_reply_t* reply);

#ifdef __cplusplus
}
#endif
