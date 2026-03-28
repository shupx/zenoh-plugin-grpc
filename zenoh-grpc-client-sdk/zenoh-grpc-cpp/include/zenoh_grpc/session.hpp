#pragma once

#include <memory>
#include <string>

extern "C" {
struct zgrpc_session_t;
zgrpc_session_t* zgrpc_open_tcp(const char* addr);
zgrpc_session_t* zgrpc_open_unix(const char* path);
void zgrpc_close(zgrpc_session_t* session);
}

namespace zenoh_grpc {

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

  private:
    void reset() {
        if (session_ != nullptr) {
            zgrpc_close(session_);
            session_ = nullptr;
        }
    }

    zgrpc_session_t* session_;
};

}  // namespace zenoh_grpc
