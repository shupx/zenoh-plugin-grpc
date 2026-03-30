#pragma once

#include <cstdint>
#include <chrono>
#include <functional>
#include <iterator>
#include <optional>
#include <stdexcept>
#include <string>
#include <thread>
#include <utility>
#include <vector>

#include <zenoh_grpc.h>

namespace zenoh_grpc {

enum class SampleKind : int {
    Unspecified = ZGRPC_SAMPLE_KIND_UNSPECIFIED,
    Put = ZGRPC_SAMPLE_KIND_PUT,
    Delete = ZGRPC_SAMPLE_KIND_DELETE,
};

enum class CongestionControl : int {
    Unspecified = ZGRPC_CONGESTION_CONTROL_UNSPECIFIED,
    Block = ZGRPC_CONGESTION_CONTROL_BLOCK,
    Drop = ZGRPC_CONGESTION_CONTROL_DROP,
};

enum class Priority : int {
    Unspecified = ZGRPC_PRIORITY_UNSPECIFIED,
    RealTime = ZGRPC_PRIORITY_REAL_TIME,
    InteractiveHigh = ZGRPC_PRIORITY_INTERACTIVE_HIGH,
    InteractiveLow = ZGRPC_PRIORITY_INTERACTIVE_LOW,
    DataHigh = ZGRPC_PRIORITY_DATA_HIGH,
    Data = ZGRPC_PRIORITY_DATA,
    DataLow = ZGRPC_PRIORITY_DATA_LOW,
    Background = ZGRPC_PRIORITY_BACKGROUND,
};

enum class Reliability : int {
    Unspecified = ZGRPC_RELIABILITY_UNSPECIFIED,
    BestEffort = ZGRPC_RELIABILITY_BEST_EFFORT,
    Reliable = ZGRPC_RELIABILITY_RELIABLE,
};

enum class Locality : int {
    Unspecified = ZGRPC_LOCALITY_UNSPECIFIED,
    Any = ZGRPC_LOCALITY_ANY,
    SessionLocal = ZGRPC_LOCALITY_SESSION_LOCAL,
    Remote = ZGRPC_LOCALITY_REMOTE,
};

enum class QueryTarget : int {
    Unspecified = ZGRPC_QUERY_TARGET_UNSPECIFIED,
    BestMatching = ZGRPC_QUERY_TARGET_BEST_MATCHING,
    All = ZGRPC_QUERY_TARGET_ALL,
    AllComplete = ZGRPC_QUERY_TARGET_ALL_COMPLETE,
};

enum class ConsolidationMode : int {
    Unspecified = ZGRPC_CONSOLIDATION_MODE_UNSPECIFIED,
    Auto = ZGRPC_CONSOLIDATION_MODE_AUTO,
    None = ZGRPC_CONSOLIDATION_MODE_NONE,
    Monotonic = ZGRPC_CONSOLIDATION_MODE_MONOTONIC,
    Latest = ZGRPC_CONSOLIDATION_MODE_LATEST,
};

struct SourceInfo {
    std::string id;
    std::uint64_t sequence = 0;
};

struct Sample {
    std::string key_expr;
    std::vector<std::uint8_t> payload;
    std::string encoding;
    SampleKind kind = SampleKind::Unspecified;
    std::vector<std::uint8_t> attachment;
    std::string timestamp;
    std::optional<SourceInfo> source_info;
};

struct SubscriberEvent {
    std::optional<Sample> sample;
};

struct ReplyError {
    std::vector<std::uint8_t> payload;
    std::string encoding;
};

struct Reply {
    bool ok = false;
    std::optional<Sample> sample;
    std::optional<ReplyError> error;
};

class Publisher;
class Subscriber;
class Queryable;
class Querier;
class ReplyStream;
class Query;

namespace detail {

inline void check_rc(int rc, const char* message) {
    if (rc != 0) {
        throw std::runtime_error(message);
    }
}

inline std::string take_string(char* value) {
    if (value == nullptr) {
        return {};
    }
    std::string out(value);
    zgrpc_string_free(value);
    return out;
}

inline std::string copy_string(const char* value) { return value == nullptr ? std::string() : std::string(value); }

inline std::vector<std::uint8_t> take_bytes(std::uint8_t* value, std::size_t len) {
    if (value == nullptr || len == 0) {
        return {};
    }
    std::vector<std::uint8_t> out(value, value + len);
    zgrpc_bytes_free(value, len);
    return out;
}

inline std::vector<std::uint8_t> copy_bytes(const std::uint8_t* value, std::size_t len) {
    if (value == nullptr || len == 0) {
        return {};
    }
    return std::vector<std::uint8_t>(value, value + len);
}

inline std::optional<SourceInfo> take_source_info(zgrpc_source_info_t& raw) {
    if (raw.is_valid == 0) {
        zgrpc_source_info_free(&raw);
        return std::nullopt;
    }
    SourceInfo info{take_string(raw.id), raw.sequence};
    raw.id = nullptr;
    zgrpc_source_info_free(&raw);
    return info;
}

inline Sample take_sample(zgrpc_sample_ex_t& raw) {
    Sample out;
    out.key_expr = take_string(raw.key_expr);
    out.payload = take_bytes(raw.payload, raw.payload_len);
    out.encoding = take_string(raw.encoding);
    out.kind = static_cast<SampleKind>(raw.kind);
    out.attachment = take_bytes(raw.attachment, raw.attachment_len);
    out.timestamp = take_string(raw.timestamp);
    out.source_info = take_source_info(raw.source_info);
    raw.key_expr = nullptr;
    raw.payload = nullptr;
    raw.payload_len = 0;
    raw.encoding = nullptr;
    raw.attachment = nullptr;
    raw.attachment_len = 0;
    raw.timestamp = nullptr;
    return out;
}

inline Sample borrow_sample(const zgrpc_sample_ex_t& raw) {
    Sample out;
    out.key_expr = copy_string(raw.key_expr);
    out.payload = copy_bytes(raw.payload, raw.payload_len);
    out.encoding = copy_string(raw.encoding);
    out.kind = static_cast<SampleKind>(raw.kind);
    out.attachment = copy_bytes(raw.attachment, raw.attachment_len);
    out.timestamp = copy_string(raw.timestamp);
    if (raw.source_info.is_valid != 0) {
        out.source_info = SourceInfo{copy_string(raw.source_info.id), raw.source_info.sequence};
    }
    return out;
}

inline SubscriberEvent take_subscriber_event(zgrpc_subscriber_event_t& raw) {
    SubscriberEvent event;
    if (raw.has_sample != 0) {
        event.sample = take_sample(raw.sample);
    }
    zgrpc_subscriber_event_free(&raw);
    return event;
}

inline SubscriberEvent borrow_subscriber_event(const zgrpc_subscriber_event_t& raw) {
    SubscriberEvent event;
    if (raw.has_sample != 0) {
        event.sample = borrow_sample(raw.sample);
    }
    return event;
}

inline ReplyError take_reply_error(zgrpc_reply_error_t& raw) {
    ReplyError out;
    out.payload = take_bytes(raw.payload, raw.payload_len);
    out.encoding = take_string(raw.encoding);
    raw.payload = nullptr;
    raw.payload_len = 0;
    raw.encoding = nullptr;
    zgrpc_reply_error_free(&raw);
    return out;
}

inline Reply take_reply(zgrpc_reply_ex_t& raw) {
    Reply reply;
    if (raw.has_sample != 0) {
        reply.ok = true;
        reply.sample = take_sample(raw.sample);
    } else if (raw.has_error != 0) {
        reply.ok = false;
        reply.error = take_reply_error(raw.error);
    }
    zgrpc_reply_ex_free(&raw);
    return reply;
}

template <class T>
inline const std::uint8_t* bytes_ptr(const std::vector<T>& value) {
    return value.empty() ? nullptr : reinterpret_cast<const std::uint8_t*>(value.data());
}

template <class Options>
inline const char* str_ptr(const Options& value) {
    return value.empty() ? nullptr : value.c_str();
}

template <class F>
struct CallbackContext {
    F callback;

    static void drop(void* context) {
        delete static_cast<CallbackContext*>(context);
    }
};

}  // namespace detail

class Query {
  public:
    struct ReplyOptions {
        std::string encoding;
        std::vector<std::uint8_t> attachment;
        std::string timestamp;
    };

    struct ReplyErrOptions {
        std::string encoding;
    };

    struct ReplyDeleteOptions {
        std::vector<std::uint8_t> attachment;
        std::string timestamp;
    };

    Query() = default;
    Query(zgrpc_queryable_t* owner, zgrpc_query_ex_t& raw, bool auto_finish)
        : owner_(owner),
          query_id_(raw.query_id),
          selector_(detail::take_string(raw.selector)),
          key_expr_(detail::take_string(raw.key_expr)),
          parameters_(detail::take_string(raw.parameters)),
          payload_(detail::take_bytes(raw.payload, raw.payload_len)),
          encoding_(detail::take_string(raw.encoding)),
          attachment_(detail::take_bytes(raw.attachment, raw.attachment_len)),
          auto_finish_(auto_finish) {}

    static Query borrow(zgrpc_queryable_t* owner, const zgrpc_query_ex_t& raw, bool auto_finish) {
        Query query;
        query.owner_ = owner;
        query.query_id_ = raw.query_id;
        query.selector_ = detail::copy_string(raw.selector);
        query.key_expr_ = detail::copy_string(raw.key_expr);
        query.parameters_ = detail::copy_string(raw.parameters);
        query.payload_ = detail::copy_bytes(raw.payload, raw.payload_len);
        query.encoding_ = detail::copy_string(raw.encoding);
        query.attachment_ = detail::copy_bytes(raw.attachment, raw.attachment_len);
        query.auto_finish_ = auto_finish;
        return query;
    }

    Query(Query&& other) noexcept { *this = std::move(other); }
    Query& operator=(Query&& other) noexcept {
        if (this != &other) {
            finish_noexcept();
            owner_ = other.owner_;
            query_id_ = other.query_id_;
            selector_ = std::move(other.selector_);
            key_expr_ = std::move(other.key_expr_);
            parameters_ = std::move(other.parameters_);
            payload_ = std::move(other.payload_);
            encoding_ = std::move(other.encoding_);
            attachment_ = std::move(other.attachment_);
            auto_finish_ = other.auto_finish_;
            other.owner_ = nullptr;
            other.query_id_ = 0;
            other.auto_finish_ = false;
        }
        return *this;
    }

    Query(const Query&) = delete;
    Query& operator=(const Query&) = delete;

    ~Query() { finish_noexcept(); }

    std::uint64_t query_id() const { return query_id_; }
    const std::string& selector() const { return selector_; }
    const std::string& key_expr() const { return key_expr_; }
    const std::string& parameters() const { return parameters_; }
    const std::vector<std::uint8_t>& payload() const { return payload_; }
    const std::string& encoding() const { return encoding_; }
    const std::vector<std::uint8_t>& attachment() const { return attachment_; }

    void reply(const std::string& key_expr, const std::vector<std::uint8_t>& payload,
               const ReplyOptions& options = ReplyOptions()) const {
        ensure_owner("query reply");
        zgrpc_query_reply_options_t raw{};
        zgrpc_query_reply_options_default(&raw);
        raw.encoding = detail::str_ptr(options.encoding);
        raw.attachment = detail::bytes_ptr(options.attachment);
        raw.attachment_len = options.attachment.size();
        raw.timestamp = detail::str_ptr(options.timestamp);
        detail::check_rc(
            zgrpc_queryable_reply_with_options(owner_, query_id_, key_expr.c_str(),
                                               detail::bytes_ptr(payload), payload.size(), &raw),
            "zgrpc_queryable_reply_with_options failed");
    }

    void reply_err(const std::vector<std::uint8_t>& payload,
                   const ReplyErrOptions& options = ReplyErrOptions()) const {
        ensure_owner("query reply_err");
        zgrpc_query_reply_err_options_t raw{};
        zgrpc_query_reply_err_options_default(&raw);
        raw.encoding = detail::str_ptr(options.encoding);
        detail::check_rc(
            zgrpc_queryable_reply_err_with_options(owner_, query_id_, detail::bytes_ptr(payload),
                                                   payload.size(), &raw),
            "zgrpc_queryable_reply_err_with_options failed");
    }

    void reply_delete(const std::string& key_expr,
                      const ReplyDeleteOptions& options = ReplyDeleteOptions()) const {
        ensure_owner("query reply_delete");
        zgrpc_query_reply_delete_options_t raw{};
        zgrpc_query_reply_delete_options_default(&raw);
        raw.attachment = detail::bytes_ptr(options.attachment);
        raw.attachment_len = options.attachment.size();
        raw.timestamp = detail::str_ptr(options.timestamp);
        detail::check_rc(
            zgrpc_queryable_reply_delete_with_options(owner_, query_id_, key_expr.c_str(), &raw),
            "zgrpc_queryable_reply_delete_with_options failed");
    }

    void finish() {
        ensure_owner("query finish");
        detail::check_rc(zgrpc_queryable_finish(owner_, query_id_), "zgrpc_queryable_finish failed");
        owner_ = nullptr;
        auto_finish_ = false;
    }

  private:
    void ensure_owner(const char* action) const {
        if (owner_ == nullptr) {
            throw std::runtime_error(std::string(action) + ": query already finished");
        }
    }

    void finish_noexcept() noexcept {
        if (owner_ != nullptr && auto_finish_) {
            (void)zgrpc_queryable_finish(owner_, query_id_);
        }
        owner_ = nullptr;
        auto_finish_ = false;
    }

    zgrpc_queryable_t* owner_ = nullptr;
    std::uint64_t query_id_ = 0;
    std::string selector_;
    std::string key_expr_;
    std::string parameters_;
    std::vector<std::uint8_t> payload_;
    std::string encoding_;
    std::vector<std::uint8_t> attachment_;
    bool auto_finish_ = false;
};

class ReplyStream {
  public:
    class Iterator {
      public:
        using iterator_category = std::input_iterator_tag;
        using value_type = Reply;
        using difference_type = std::ptrdiff_t;
        using pointer = const Reply*;
        using reference = const Reply&;

        Iterator() = default;
        explicit Iterator(const ReplyStream* stream) : stream_(stream) { advance(); }

        reference operator*() const { return *current_; }
        pointer operator->() const { return &(*current_); }

        Iterator& operator++() {
            advance();
            return *this;
        }

        Iterator operator++(int) {
            Iterator copy(*this);
            advance();
            return copy;
        }

        bool operator==(const Iterator& other) const {
            const bool at_end = stream_ == nullptr;
            const bool other_at_end = other.stream_ == nullptr;
            if (at_end || other_at_end) {
                return at_end == other_at_end;
            }
            return stream_ == other.stream_;
        }

        bool operator!=(const Iterator& other) const { return !(*this == other); }

      private:
        void advance() {
            if (stream_ == nullptr) {
                current_.reset();
                return;
            }
            current_ = stream_->next_for_iteration();
            if (!current_.has_value()) {
                stream_ = nullptr;
            }
        }

        const ReplyStream* stream_ = nullptr;
        std::optional<Reply> current_;
    };

    ReplyStream() = default;
    explicit ReplyStream(zgrpc_reply_stream_t* inner) : inner_(inner) {}
    ReplyStream(ReplyStream&& other) noexcept : inner_(other.inner_) { other.inner_ = nullptr; }
    ReplyStream& operator=(ReplyStream&& other) noexcept {
        if (this != &other) {
            reset();
            inner_ = other.inner_;
            other.inner_ = nullptr;
        }
        return *this;
    }
    ReplyStream(const ReplyStream&) = delete;
    ReplyStream& operator=(const ReplyStream&) = delete;
    ~ReplyStream() { reset(); }

    bool valid() const { return inner_ != nullptr; }

    Reply recv() const {
        zgrpc_reply_ex_t raw{};
        detail::check_rc(zgrpc_reply_stream_recv(inner_, &raw), "zgrpc_reply_stream_recv failed");
        return detail::take_reply(raw);
    }

    std::optional<Reply> try_recv() const {
        zgrpc_reply_ex_t raw{};
        const int rc = zgrpc_reply_stream_try_recv(inner_, &raw);
        if (rc == 1) {
            return std::nullopt;
        }
        detail::check_rc(rc, "zgrpc_reply_stream_try_recv failed");
        return detail::take_reply(raw);
    }

    std::uint64_t dropped_count() const { return zgrpc_reply_stream_dropped_count(inner_); }
    bool is_closed() const { return zgrpc_reply_stream_is_closed(inner_) != 0; }

    Iterator begin() const { return Iterator(this); }
    Iterator end() const { return Iterator(); }

  private:
    std::optional<Reply> next_for_iteration() const {
        while (valid()) {
            auto reply = try_recv();
            if (reply.has_value()) {
                return reply;
            }
            if (is_closed()) {
                break;
            }
            std::this_thread::sleep_for(std::chrono::milliseconds(10));
        }
        return std::nullopt;
    }

    void reset() {
        if (inner_ != nullptr) {
            zgrpc_reply_stream_drop(inner_);
            inner_ = nullptr;
        }
    }

    zgrpc_reply_stream_t* inner_ = nullptr;
};

class Publisher {
  public:
    struct PutOptions {
        std::string encoding;
        std::vector<std::uint8_t> attachment;
        std::string timestamp;
    };

    struct DeleteOptions {
        std::vector<std::uint8_t> attachment;
        std::string timestamp;
    };

    Publisher() = default;
    explicit Publisher(zgrpc_publisher_t* inner) : inner_(inner) {}
    Publisher(Publisher&& other) noexcept : inner_(other.inner_) { other.inner_ = nullptr; }
    Publisher& operator=(Publisher&& other) noexcept {
        if (this != &other) {
            reset();
            inner_ = other.inner_;
            other.inner_ = nullptr;
        }
        return *this;
    }
    Publisher(const Publisher&) = delete;
    Publisher& operator=(const Publisher&) = delete;
    ~Publisher() { reset(); }

    bool valid() const { return inner_ != nullptr; }

    void put(const std::vector<std::uint8_t>& payload, const PutOptions& options = PutOptions()) const {
        zgrpc_publisher_put_options_t raw{};
        zgrpc_publisher_put_options_default(&raw);
        raw.encoding = detail::str_ptr(options.encoding);
        raw.attachment = detail::bytes_ptr(options.attachment);
        raw.attachment_len = options.attachment.size();
        raw.timestamp = detail::str_ptr(options.timestamp);
        detail::check_rc(
            zgrpc_publisher_put_with_options(inner_, detail::bytes_ptr(payload), payload.size(), &raw),
            "zgrpc_publisher_put_with_options failed");
    }

    void delete_resource(const DeleteOptions& options = DeleteOptions()) const {
        zgrpc_publisher_delete_options_t raw{};
        zgrpc_publisher_delete_options_default(&raw);
        raw.attachment = detail::bytes_ptr(options.attachment);
        raw.attachment_len = options.attachment.size();
        raw.timestamp = detail::str_ptr(options.timestamp);
        detail::check_rc(zgrpc_publisher_delete_with_options(inner_, &raw),
                         "zgrpc_publisher_delete_with_options failed");
    }

    std::uint64_t send_dropped_count() const { return zgrpc_publisher_send_dropped_count(inner_); }

    void undeclare() {
        detail::check_rc(zgrpc_publisher_undeclare(inner_), "zgrpc_publisher_undeclare failed");
        inner_ = nullptr;
    }

  private:
    void reset() {
        if (inner_ != nullptr) {
            (void)zgrpc_publisher_undeclare(inner_);
            inner_ = nullptr;
        }
    }

    zgrpc_publisher_t* inner_ = nullptr;
};

class Subscriber {
  public:
    Subscriber() = default;
    explicit Subscriber(zgrpc_subscriber_t* inner) : inner_(inner) {}
    Subscriber(Subscriber&& other) noexcept : inner_(other.inner_) { other.inner_ = nullptr; }
    Subscriber& operator=(Subscriber&& other) noexcept {
        if (this != &other) {
            reset();
            inner_ = other.inner_;
            other.inner_ = nullptr;
        }
        return *this;
    }
    Subscriber(const Subscriber&) = delete;
    Subscriber& operator=(const Subscriber&) = delete;
    ~Subscriber() { reset(); }

    bool valid() const { return inner_ != nullptr; }

    SubscriberEvent recv_event() const {
        zgrpc_subscriber_event_t raw{};
        detail::check_rc(zgrpc_subscriber_recv_event(inner_, &raw), "zgrpc_subscriber_recv_event failed");
        return detail::take_subscriber_event(raw);
    }

    std::optional<SubscriberEvent> try_recv_event() const {
        zgrpc_subscriber_event_t raw{};
        const int rc = zgrpc_subscriber_try_recv_event(inner_, &raw);
        if (rc == 1) {
            return std::nullopt;
        }
        detail::check_rc(rc, "zgrpc_subscriber_try_recv_event failed");
        return detail::take_subscriber_event(raw);
    }

    Sample recv() const {
        auto event = recv_event();
        if (!event.sample.has_value()) {
            throw std::runtime_error("subscriber event missing sample");
        }
        return std::move(*event.sample);
    }

    std::optional<Sample> try_recv() const {
        auto event = try_recv_event();
        if (!event.has_value() || !event->sample.has_value()) {
            return std::nullopt;
        }
        return std::move(*event->sample);
    }

    std::uint64_t dropped_count() const { return zgrpc_subscriber_dropped_count(inner_); }

    void undeclare() {
        detail::check_rc(zgrpc_subscriber_undeclare(inner_), "zgrpc_subscriber_undeclare failed");
        inner_ = nullptr;
    }

  private:
    void reset() {
        if (inner_ != nullptr) {
            (void)zgrpc_subscriber_undeclare(inner_);
            inner_ = nullptr;
        }
    }

    zgrpc_subscriber_t* inner_ = nullptr;
};

class Queryable {
  public:
    Queryable() = default;
    explicit Queryable(zgrpc_queryable_t* inner) : inner_(inner) {}
    Queryable(Queryable&& other) noexcept : inner_(other.inner_) { other.inner_ = nullptr; }
    Queryable& operator=(Queryable&& other) noexcept {
        if (this != &other) {
            reset();
            inner_ = other.inner_;
            other.inner_ = nullptr;
        }
        return *this;
    }
    Queryable(const Queryable&) = delete;
    Queryable& operator=(const Queryable&) = delete;
    ~Queryable() { reset(); }

    bool valid() const { return inner_ != nullptr; }

    Query recv() const {
        zgrpc_query_ex_t raw{};
        detail::check_rc(zgrpc_queryable_recv_ex(inner_, &raw), "zgrpc_queryable_recv_ex failed");
        return Query(inner_, raw, true);
    }

    std::optional<Query> try_recv() const {
        zgrpc_query_ex_t raw{};
        const int rc = zgrpc_queryable_try_recv_ex(inner_, &raw);
        if (rc == 1) {
            return std::nullopt;
        }
        detail::check_rc(rc, "zgrpc_queryable_try_recv_ex failed");
        return Query(inner_, raw, true);
    }

    std::uint64_t dropped_count() const {
        std::uint64_t count = 0;
        detail::check_rc(zgrpc_queryable_dropped_count(inner_, &count), "zgrpc_queryable_dropped_count failed");
        return count;
    }

    bool is_closed() const {
        int closed = 0;
        detail::check_rc(zgrpc_queryable_is_closed(inner_, &closed), "zgrpc_queryable_is_closed failed");
        return closed != 0;
    }

    std::uint64_t send_dropped_count() const { return zgrpc_queryable_send_dropped_count(inner_); }

    void undeclare() {
        detail::check_rc(zgrpc_queryable_undeclare(inner_), "zgrpc_queryable_undeclare failed");
        inner_ = nullptr;
    }

  private:
    void reset() {
        if (inner_ != nullptr) {
            (void)zgrpc_queryable_undeclare(inner_);
            inner_ = nullptr;
        }
    }

    zgrpc_queryable_t* inner_ = nullptr;
};

class Querier {
  public:
    struct GetOptions {
        std::vector<std::uint8_t> payload;
        std::string encoding;
        std::vector<std::uint8_t> attachment;
    };

    Querier() = default;
    explicit Querier(zgrpc_querier_t* inner) : inner_(inner) {}
    Querier(Querier&& other) noexcept : inner_(other.inner_) { other.inner_ = nullptr; }
    Querier& operator=(Querier&& other) noexcept {
        if (this != &other) {
            reset();
            inner_ = other.inner_;
            other.inner_ = nullptr;
        }
        return *this;
    }
    Querier(const Querier&) = delete;
    Querier& operator=(const Querier&) = delete;
    ~Querier() { reset(); }

    bool valid() const { return inner_ != nullptr; }

    ReplyStream get_stream(const std::string& parameters = std::string(),
                          const GetOptions& options = GetOptions()) const {
        zgrpc_querier_get_options_t raw{};
        zgrpc_querier_get_options_default(&raw);
        raw.payload = detail::bytes_ptr(options.payload);
        raw.payload_len = options.payload.size();
        raw.encoding = detail::str_ptr(options.encoding);
        raw.attachment = detail::bytes_ptr(options.attachment);
        raw.attachment_len = options.attachment.size();
        auto* stream = zgrpc_querier_get_stream(inner_, parameters.c_str(), &raw);
        if (stream == nullptr) {
            throw std::runtime_error("zgrpc_querier_get_stream failed");
        }
        return ReplyStream(stream);
    }

    Reply get(const std::string& parameters = std::string(),
              const GetOptions& options = GetOptions()) const {
        auto stream = get_stream(parameters, options);
        return stream.recv();
    }

    void undeclare() {
        detail::check_rc(zgrpc_querier_undeclare(inner_), "zgrpc_querier_undeclare failed");
        inner_ = nullptr;
    }

  private:
    void reset() {
        if (inner_ != nullptr) {
            (void)zgrpc_querier_undeclare(inner_);
            inner_ = nullptr;
        }
    }

    zgrpc_querier_t* inner_ = nullptr;
};

class Session {
  public:
    struct PutOptions {
        std::string encoding;
        CongestionControl congestion_control = CongestionControl::Unspecified;
        Priority priority = Priority::Unspecified;
        bool express = false;
        std::vector<std::uint8_t> attachment;
        std::string timestamp;
        Locality allowed_destination = Locality::Unspecified;
    };

    struct DeleteOptions {
        CongestionControl congestion_control = CongestionControl::Unspecified;
        Priority priority = Priority::Unspecified;
        bool express = false;
        std::vector<std::uint8_t> attachment;
        std::string timestamp;
        Locality allowed_destination = Locality::Unspecified;
    };

    struct GetOptions {
        QueryTarget target = QueryTarget::Unspecified;
        ConsolidationMode consolidation = ConsolidationMode::Unspecified;
        std::uint64_t timeout_ms = 0;
        std::vector<std::uint8_t> payload;
        std::string encoding;
        std::vector<std::uint8_t> attachment;
        Locality allowed_destination = Locality::Unspecified;
    };

    struct PublisherOptions {
        std::string encoding;
        CongestionControl congestion_control = CongestionControl::Unspecified;
        Priority priority = Priority::Unspecified;
        bool express = false;
        Reliability reliability = Reliability::Unspecified;
        Locality allowed_destination = Locality::Unspecified;
    };

    struct SubscriberOptions {
        Locality allowed_origin = Locality::Unspecified;
    };

    struct QueryableOptions {
        bool complete = false;
        Locality allowed_origin = Locality::Unspecified;
    };

    struct QuerierOptions {
        QueryTarget target = QueryTarget::Unspecified;
        ConsolidationMode consolidation = ConsolidationMode::Unspecified;
        std::uint64_t timeout_ms = 0;
        Locality allowed_destination = Locality::Unspecified;
    };

    using SubscriberCallback = std::function<void(const SubscriberEvent&)>;
    using QueryableCallback = std::function<void(Query)>;

    Session() = default;
    explicit Session(zgrpc_session_t* inner) : inner_(inner) {}
    Session(Session&& other) noexcept : inner_(other.inner_) { other.inner_ = nullptr; }
    Session& operator=(Session&& other) noexcept {
        if (this != &other) {
            reset();
            inner_ = other.inner_;
            other.inner_ = nullptr;
        }
        return *this;
    }
    Session(const Session&) = delete;
    Session& operator=(const Session&) = delete;
    ~Session() { reset(); }

    static Session connect(const std::string& endpoint = "unix:///tmp/zenoh-grpc.sock") {
        auto* raw = zgrpc_session_connect(endpoint.c_str());
        if (raw == nullptr) {
            throw std::runtime_error("zgrpc_session_connect failed");
        }
        return Session(raw);
    }

    static Session open_tcp(const std::string& addr) {
        auto* raw = zgrpc_open_tcp(addr.c_str());
        if (raw == nullptr) {
            throw std::runtime_error("zgrpc_open_tcp failed");
        }
        return Session(raw);
    }

    static Session open_unix(const std::string& path) {
        auto* raw = zgrpc_open_unix(path.c_str());
        if (raw == nullptr) {
            throw std::runtime_error("zgrpc_open_unix failed");
        }
        return Session(raw);
    }

    bool valid() const { return inner_ != nullptr; }

    std::string info() const {
        char* value = zgrpc_session_info(inner_);
        if (value == nullptr) {
            throw std::runtime_error("zgrpc_session_info failed");
        }
        return detail::take_string(value);
    }

    void close() {
        if (inner_ != nullptr) {
            zgrpc_close(inner_);
            inner_ = nullptr;
        }
    }

    void put(const std::string& key_expr, const std::vector<std::uint8_t>& payload) const {
        put(key_expr, payload, PutOptions{});
    }

    void put(const std::string& key_expr, const std::vector<std::uint8_t>& payload,
             const PutOptions& options) const {
        zgrpc_session_put_options_t raw{};
        zgrpc_session_put_options_default(&raw);
        raw.encoding = detail::str_ptr(options.encoding);
        raw.congestion_control = static_cast<int>(options.congestion_control);
        raw.priority = static_cast<int>(options.priority);
        raw.express = options.express;
        raw.attachment = detail::bytes_ptr(options.attachment);
        raw.attachment_len = options.attachment.size();
        raw.timestamp = detail::str_ptr(options.timestamp);
        raw.allowed_destination = static_cast<int>(options.allowed_destination);
        detail::check_rc(zgrpc_session_put_with_options(inner_, key_expr.c_str(), detail::bytes_ptr(payload),
                                                        payload.size(), &raw),
                         "zgrpc_session_put_with_options failed");
    }

    void delete_resource(const std::string& key_expr) const { delete_resource(key_expr, DeleteOptions{}); }

    void delete_resource(const std::string& key_expr, const DeleteOptions& options) const {
        zgrpc_session_delete_options_t raw{};
        zgrpc_session_delete_options_default(&raw);
        raw.congestion_control = static_cast<int>(options.congestion_control);
        raw.priority = static_cast<int>(options.priority);
        raw.express = options.express;
        raw.attachment = detail::bytes_ptr(options.attachment);
        raw.attachment_len = options.attachment.size();
        raw.timestamp = detail::str_ptr(options.timestamp);
        raw.allowed_destination = static_cast<int>(options.allowed_destination);
        detail::check_rc(zgrpc_session_delete_with_options(inner_, key_expr.c_str(), &raw),
                         "zgrpc_session_delete_with_options failed");
    }

    ReplyStream get(const std::string& selector) const { return get(selector, GetOptions{}); }

    ReplyStream get(const std::string& selector, const GetOptions& options) const {
        zgrpc_session_get_options_t raw{};
        zgrpc_session_get_options_default(&raw);
        raw.target = static_cast<int>(options.target);
        raw.consolidation = static_cast<int>(options.consolidation);
        raw.timeout_ms = options.timeout_ms;
        raw.payload = detail::bytes_ptr(options.payload);
        raw.payload_len = options.payload.size();
        raw.encoding = detail::str_ptr(options.encoding);
        raw.attachment = detail::bytes_ptr(options.attachment);
        raw.attachment_len = options.attachment.size();
        raw.allowed_destination = static_cast<int>(options.allowed_destination);
        auto* stream = zgrpc_session_get(inner_, selector.c_str(), &raw);
        if (stream == nullptr) {
            throw std::runtime_error("zgrpc_session_get failed");
        }
        return ReplyStream(stream);
    }

    Publisher declare_publisher(const std::string& key_expr) const {
        return declare_publisher(key_expr, PublisherOptions{});
    }

    Publisher declare_publisher(const std::string& key_expr, const PublisherOptions& options) const {
        zgrpc_publisher_options_t raw{};
        zgrpc_publisher_options_default(&raw);
        raw.encoding = detail::str_ptr(options.encoding);
        raw.congestion_control = static_cast<int>(options.congestion_control);
        raw.priority = static_cast<int>(options.priority);
        raw.express = options.express;
        raw.reliability = static_cast<int>(options.reliability);
        raw.allowed_destination = static_cast<int>(options.allowed_destination);
        auto* publisher = zgrpc_declare_publisher_with_options(inner_, key_expr.c_str(), &raw);
        if (publisher == nullptr) {
            throw std::runtime_error("zgrpc_declare_publisher_with_options failed");
        }
        return Publisher(publisher);
    }

    Subscriber declare_subscriber(const std::string& key_expr) const {
        return declare_subscriber(key_expr, SubscriberOptions{});
    }

    Subscriber declare_subscriber(const std::string& key_expr, const SubscriberOptions& options) const {
        zgrpc_subscriber_options_t raw{};
        zgrpc_subscriber_options_default(&raw);
        raw.allowed_origin = static_cast<int>(options.allowed_origin);
        auto* subscriber = zgrpc_declare_subscriber_with_options(inner_, key_expr.c_str(), &raw);
        if (subscriber == nullptr) {
            throw std::runtime_error("zgrpc_declare_subscriber_with_options failed");
        }
        return Subscriber(subscriber);
    }

    Subscriber declare_subscriber(const std::string& key_expr, SubscriberCallback callback) const {
        return declare_subscriber(key_expr, std::move(callback), SubscriberOptions{});
    }

    Subscriber declare_subscriber(const std::string& key_expr, SubscriberCallback callback,
                                  const SubscriberOptions& options) const {
        struct Context : detail::CallbackContext<SubscriberCallback> {
            static void on_event(const zgrpc_subscriber_event_t* event, void* context) {
                auto* self = static_cast<Context*>(context);
                self->callback(detail::borrow_subscriber_event(*event));
            }
        };
        auto* context = new Context{std::move(callback)};
        zgrpc_subscriber_options_t raw{};
        zgrpc_subscriber_options_default(&raw);
        raw.allowed_origin = static_cast<int>(options.allowed_origin);
        auto* subscriber = zgrpc_declare_subscriber_with_callback(
            inner_, key_expr.c_str(), &Context::on_event, context, &Context::drop, &raw);
        if (subscriber == nullptr) {
            throw std::runtime_error("zgrpc_declare_subscriber_with_callback failed");
        }
        return Subscriber(subscriber);
    }

    Queryable declare_queryable(const std::string& key_expr) const {
        return declare_queryable(key_expr, QueryableOptions{});
    }

    Queryable declare_queryable(const std::string& key_expr, const QueryableOptions& options) const {
        zgrpc_queryable_options_t raw{};
        zgrpc_queryable_options_default(&raw);
        raw.complete = options.complete;
        raw.allowed_origin = static_cast<int>(options.allowed_origin);
        auto* queryable = zgrpc_declare_queryable_with_options(inner_, key_expr.c_str(), &raw);
        if (queryable == nullptr) {
            throw std::runtime_error("zgrpc_declare_queryable_with_options failed");
        }
        return Queryable(queryable);
    }

    Queryable declare_queryable(const std::string& key_expr, QueryableCallback callback) const {
        return declare_queryable(key_expr, std::move(callback), QueryableOptions{});
    }

    Queryable declare_queryable(const std::string& key_expr, QueryableCallback callback,
                                const QueryableOptions& options) const {
        struct Context : detail::CallbackContext<QueryableCallback> {
            static void on_query(zgrpc_queryable_t* owner, const zgrpc_query_ex_t* query, void* context) {
                auto* self = static_cast<Context*>(context);
                self->callback(Query::borrow(owner, *query, true));
            }
        };
        auto* context = new Context{std::move(callback)};
        zgrpc_queryable_options_t raw{};
        zgrpc_queryable_options_default(&raw);
        raw.complete = options.complete;
        raw.allowed_origin = static_cast<int>(options.allowed_origin);
        auto* queryable = zgrpc_declare_queryable_with_callback(
            inner_, key_expr.c_str(), &Context::on_query, context, &Context::drop, &raw);
        if (queryable == nullptr) {
            throw std::runtime_error("zgrpc_declare_queryable_with_callback failed");
        }
        return Queryable(queryable);
    }

    Querier declare_querier(const std::string& key_expr) const {
        return declare_querier(key_expr, QuerierOptions{});
    }

    Querier declare_querier(const std::string& key_expr, const QuerierOptions& options) const {
        zgrpc_querier_options_t raw{};
        zgrpc_querier_options_default(&raw);
        raw.target = static_cast<int>(options.target);
        raw.consolidation = static_cast<int>(options.consolidation);
        raw.timeout_ms = options.timeout_ms;
        raw.allowed_destination = static_cast<int>(options.allowed_destination);
        auto* querier = zgrpc_declare_querier_with_options(inner_, key_expr.c_str(), &raw);
        if (querier == nullptr) {
            throw std::runtime_error("zgrpc_declare_querier_with_options failed");
        }
        return Querier(querier);
    }

  private:
    void reset() {
        if (inner_ != nullptr) {
            zgrpc_close(inner_);
            inner_ = nullptr;
        }
    }

    zgrpc_session_t* inner_ = nullptr;
};

}  // namespace zenoh_grpc
