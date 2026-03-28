#pragma once

#include <cstdint>
#include <stdexcept>
#include <string>
#include <vector>

extern "C" {
struct zgrpc_session_t;
struct zgrpc_publisher_t;
struct zgrpc_subscriber_t;
struct zgrpc_queryable_t;
struct zgrpc_querier_t;
struct zgrpc_sample_t {
    char* key_expr;
    std::uint8_t* payload;
    std::size_t payload_len;
};
struct zgrpc_query_t {
    std::uint64_t query_id;
    char* key_expr;
    char* parameters;
    std::uint8_t* payload;
    std::size_t payload_len;
};
struct zgrpc_reply_t {
    int is_error;
    char* key_expr;
    std::uint8_t* payload;
    std::size_t payload_len;
};
zgrpc_session_t* zgrpc_open_tcp(const char* addr);
zgrpc_session_t* zgrpc_open_unix(const char* path);
void zgrpc_close(zgrpc_session_t* session);
int zgrpc_session_put(zgrpc_session_t* session, const char* key_expr, const std::uint8_t* payload, std::size_t payload_len);
int zgrpc_session_delete(zgrpc_session_t* session, const char* key_expr);
zgrpc_publisher_t* zgrpc_declare_publisher(zgrpc_session_t* session, const char* key_expr);
int zgrpc_publisher_put(zgrpc_publisher_t* publisher, const std::uint8_t* payload, std::size_t payload_len);
int zgrpc_publisher_delete(zgrpc_publisher_t* publisher);
int zgrpc_publisher_undeclare(zgrpc_publisher_t* publisher);
zgrpc_subscriber_t* zgrpc_declare_subscriber(zgrpc_session_t* session, const char* key_expr);
int zgrpc_subscriber_recv(zgrpc_subscriber_t* subscriber, zgrpc_sample_t* sample_out);
int zgrpc_subscriber_try_recv(zgrpc_subscriber_t* subscriber, zgrpc_sample_t* sample_out);
int zgrpc_subscriber_undeclare(zgrpc_subscriber_t* subscriber);
zgrpc_queryable_t* zgrpc_declare_queryable(zgrpc_session_t* session, const char* key_expr);
int zgrpc_queryable_recv(zgrpc_queryable_t* queryable, zgrpc_query_t* query_out);
int zgrpc_queryable_reply(zgrpc_queryable_t* queryable, std::uint64_t query_id, const char* key_expr, const std::uint8_t* payload, std::size_t payload_len);
int zgrpc_queryable_reply_err(zgrpc_queryable_t* queryable, std::uint64_t query_id, const std::uint8_t* payload, std::size_t payload_len);
int zgrpc_queryable_reply_delete(zgrpc_queryable_t* queryable, std::uint64_t query_id, const char* key_expr);
int zgrpc_queryable_undeclare(zgrpc_queryable_t* queryable);
zgrpc_querier_t* zgrpc_declare_querier(zgrpc_session_t* session, const char* key_expr);
int zgrpc_querier_get(zgrpc_querier_t* querier, const char* parameters, zgrpc_reply_t* reply_out);
int zgrpc_querier_undeclare(zgrpc_querier_t* querier);
void zgrpc_sample_free(zgrpc_sample_t* sample);
void zgrpc_query_free(zgrpc_query_t* query);
void zgrpc_reply_free(zgrpc_reply_t* reply);
}

namespace zenoh_grpc {

class Publisher;
class Subscriber;
class Queryable;
class Querier;

struct Sample {
    std::string key_expr;
    std::vector<std::uint8_t> payload;
};

struct Query {
    std::uint64_t query_id;
    std::string key_expr;
    std::string parameters;
    std::vector<std::uint8_t> payload;
};

struct Reply {
    bool is_error;
    std::string key_expr;
    std::vector<std::uint8_t> payload;
};

class Session {
  public:
    static Session open_tcp(const std::string& addr) { return Session(zgrpc_open_tcp(addr.c_str())); }
    static Session open_unix(const std::string& path) { return Session(zgrpc_open_unix(path.c_str())); }

    Session() : session_(nullptr) {}
    explicit Session(zgrpc_session_t* session) : session_(session) {}
    Session(Session&& other) noexcept : session_(other.session_) { other.session_ = nullptr; }
    Session& operator=(Session&& other) noexcept {
        if (this != &other) {
            reset();
            session_ = other.session_;
            other.session_ = nullptr;
        }
        return *this;
    }
    Session(const Session&) = delete;
    Session& operator=(const Session&) = delete;
    ~Session() { reset(); }

    bool valid() const { return session_ != nullptr; }
    void put(const std::string& key_expr, const std::vector<std::uint8_t>& payload) const {
        if (zgrpc_session_put(session_, key_expr.c_str(), payload.data(), payload.size()) != 0) {
            throw std::runtime_error("zgrpc_session_put failed");
        }
    }
    void delete_resource(const std::string& key_expr) const {
        if (zgrpc_session_delete(session_, key_expr.c_str()) != 0) {
            throw std::runtime_error("zgrpc_session_delete failed");
        }
    }
    Publisher declare_publisher(const std::string& key_expr) const;
    Subscriber declare_subscriber(const std::string& key_expr) const;
    Queryable declare_queryable(const std::string& key_expr) const;
    Querier declare_querier(const std::string& key_expr) const;

  private:
    friend class Publisher;
    friend class Subscriber;
    friend class Queryable;
    friend class Querier;
    void reset() {
        if (session_ != nullptr) {
            zgrpc_close(session_);
            session_ = nullptr;
        }
    }

    zgrpc_session_t* session_;
};

class Publisher {
  public:
    explicit Publisher(zgrpc_publisher_t* publisher = nullptr) : publisher_(publisher) {}
    Publisher(Publisher&& other) noexcept : publisher_(other.publisher_) { other.publisher_ = nullptr; }
    Publisher& operator=(Publisher&& other) noexcept {
        if (this != &other) {
            reset();
            publisher_ = other.publisher_;
            other.publisher_ = nullptr;
        }
        return *this;
    }
    Publisher(const Publisher&) = delete;
    Publisher& operator=(const Publisher&) = delete;
    ~Publisher() { reset(); }

    void put(const std::vector<std::uint8_t>& payload) const {
        if (zgrpc_publisher_put(publisher_, payload.data(), payload.size()) != 0) {
            throw std::runtime_error("zgrpc_publisher_put failed");
        }
    }
    void delete_resource() const {
        if (zgrpc_publisher_delete(publisher_) != 0) {
            throw std::runtime_error("zgrpc_publisher_delete failed");
        }
    }

  private:
    void reset() {
        if (publisher_ != nullptr) {
            zgrpc_publisher_undeclare(publisher_);
            publisher_ = nullptr;
        }
    }

    zgrpc_publisher_t* publisher_;
};

class Subscriber {
  public:
    explicit Subscriber(zgrpc_subscriber_t* subscriber = nullptr) : subscriber_(subscriber) {}
    Subscriber(Subscriber&& other) noexcept : subscriber_(other.subscriber_) { other.subscriber_ = nullptr; }
    Subscriber& operator=(Subscriber&& other) noexcept {
        if (this != &other) {
            reset();
            subscriber_ = other.subscriber_;
            other.subscriber_ = nullptr;
        }
        return *this;
    }
    Subscriber(const Subscriber&) = delete;
    Subscriber& operator=(const Subscriber&) = delete;
    ~Subscriber() { reset(); }

    Sample recv() const {
        zgrpc_sample_t raw{nullptr, nullptr, 0};
        if (zgrpc_subscriber_recv(subscriber_, &raw) != 0) {
            throw std::runtime_error("zgrpc_subscriber_recv failed");
        }
        Sample sample{
            raw.key_expr != nullptr ? std::string(raw.key_expr) : std::string(),
            std::vector<std::uint8_t>(raw.payload, raw.payload + raw.payload_len),
        };
        zgrpc_sample_free(&raw);
        return sample;
    }

  private:
    void reset() {
        if (subscriber_ != nullptr) {
            zgrpc_subscriber_undeclare(subscriber_);
            subscriber_ = nullptr;
        }
    }

    zgrpc_subscriber_t* subscriber_;
};

class Queryable {
  public:
    explicit Queryable(zgrpc_queryable_t* queryable = nullptr) : queryable_(queryable) {}
    Queryable(Queryable&& other) noexcept : queryable_(other.queryable_) { other.queryable_ = nullptr; }
    Queryable& operator=(Queryable&& other) noexcept {
        if (this != &other) {
            reset();
            queryable_ = other.queryable_;
            other.queryable_ = nullptr;
        }
        return *this;
    }
    Queryable(const Queryable&) = delete;
    Queryable& operator=(const Queryable&) = delete;
    ~Queryable() { reset(); }

    Query recv() const {
        zgrpc_query_t raw{0, nullptr, nullptr, nullptr, 0};
        if (zgrpc_queryable_recv(queryable_, &raw) != 0) {
            throw std::runtime_error("zgrpc_queryable_recv failed");
        }
        Query query{
            raw.query_id,
            raw.key_expr != nullptr ? std::string(raw.key_expr) : std::string(),
            raw.parameters != nullptr ? std::string(raw.parameters) : std::string(),
            std::vector<std::uint8_t>(raw.payload, raw.payload + raw.payload_len),
        };
        zgrpc_query_free(&raw);
        return query;
    }

    void reply(std::uint64_t query_id, const std::string& key_expr, const std::vector<std::uint8_t>& payload) const {
        if (zgrpc_queryable_reply(queryable_, query_id, key_expr.c_str(), payload.data(), payload.size()) != 0) {
            throw std::runtime_error("zgrpc_queryable_reply failed");
        }
    }

  private:
    void reset() {
        if (queryable_ != nullptr) {
            zgrpc_queryable_undeclare(queryable_);
            queryable_ = nullptr;
        }
    }

    zgrpc_queryable_t* queryable_;
};

class Querier {
  public:
    explicit Querier(zgrpc_querier_t* querier = nullptr) : querier_(querier) {}
    Querier(Querier&& other) noexcept : querier_(other.querier_) { other.querier_ = nullptr; }
    Querier& operator=(Querier&& other) noexcept {
        if (this != &other) {
            reset();
            querier_ = other.querier_;
            other.querier_ = nullptr;
        }
        return *this;
    }
    Querier(const Querier&) = delete;
    Querier& operator=(const Querier&) = delete;
    ~Querier() { reset(); }

    Reply get(const std::string& parameters = "") const {
        zgrpc_reply_t raw{0, nullptr, nullptr, 0};
        if (zgrpc_querier_get(querier_, parameters.c_str(), &raw) != 0) {
            throw std::runtime_error("zgrpc_querier_get failed");
        }
        Reply reply{
            raw.is_error != 0,
            raw.key_expr != nullptr ? std::string(raw.key_expr) : std::string(),
            std::vector<std::uint8_t>(raw.payload, raw.payload + raw.payload_len),
        };
        zgrpc_reply_free(&raw);
        return reply;
    }

  private:
    void reset() {
        if (querier_ != nullptr) {
            zgrpc_querier_undeclare(querier_);
            querier_ = nullptr;
        }
    }

    zgrpc_querier_t* querier_;
};

inline Publisher Session::declare_publisher(const std::string& key_expr) const {
    return Publisher(zgrpc_declare_publisher(session_, key_expr.c_str()));
}

inline Subscriber Session::declare_subscriber(const std::string& key_expr) const {
    return Subscriber(zgrpc_declare_subscriber(session_, key_expr.c_str()));
}

inline Queryable Session::declare_queryable(const std::string& key_expr) const {
    return Queryable(zgrpc_declare_queryable(session_, key_expr.c_str()));
}

inline Querier Session::declare_querier(const std::string& key_expr) const {
    return Querier(zgrpc_declare_querier(session_, key_expr.c_str()));
}

}  // namespace zenoh_grpc
