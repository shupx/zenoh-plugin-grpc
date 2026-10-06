"""Process-level checks for a bridge or a dynamically loaded zenohd plugin."""

import argparse
import contextlib
import json
from pathlib import Path
import queue
import socket
import subprocess
import tempfile
import time

import zenoh
import zenoh_grpc as grpc


def free_port():
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


def eventually(read, timeout=10):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        value = read()
        if value is not None:
            return value
        time.sleep(0.02)
    raise AssertionError("timed out waiting for an event")


@contextlib.contextmanager
def process(command, log_path):
    with open(log_path, "w+") as log:
        child = subprocess.Popen(command, stdout=log, stderr=subprocess.STDOUT)
        try:
            yield child
        finally:
            child.terminate()
            try:
                child.wait(timeout=5)
            except subprocess.TimeoutExpired:
                child.kill()
                child.wait(timeout=5)


def check_python(tcp_endpoint, uds_endpoint, native_endpoint):
    config = zenoh.Config.from_json5(json.dumps({
        "mode": "client",
        "connect": {"endpoints": [native_endpoint]},
        "scouting": {"multicast": {"enabled": False}},
    }))
    with zenoh.open(config) as native, grpc.Session.connect(tcp_endpoint) as tcp, \
            grpc.Session.connect(uds_endpoint) as uds:
        assert tcp.info() == uds.info()
        # Test routing across a real Zenoh transport, including metadata and deletes.
        incoming = queue.Queue()
        with native.declare_subscriber("compat/sdk/**", incoming.put), \
                uds.declare_publisher("compat/sdk/value", encoding="text/plain") as pub:
            time.sleep(0.3)
            pub.put(b"python-to-native", attachment=b"metadata")
            sample = incoming.get(timeout=10)
            assert sample.payload.to_bytes() == b"python-to-native"
            assert sample.attachment.to_bytes() == b"metadata"
            assert str(sample.encoding) == "text/plain"
            pub.delete()
            assert incoming.get(timeout=10).kind == zenoh.SampleKind.DELETE

        with tcp.declare_subscriber("compat/python/**") as sub:
            time.sleep(0.3)
            native.put("compat/python/value", b"native-to-python", encoding="text/plain")
            sample = eventually(sub.try_recv)
            assert bytes(sample.payload) == b"native-to-python"
            assert sample.encoding == "text/plain"
            native.delete("compat/python/value")
            assert eventually(sub.try_recv).kind == grpc.SampleKind.DELETE

        def reply(query):
            assert bytes(query.payload) == b"request"
            query.reply(query.key_expr, b"python-reply", encoding="text/plain")
            query.drop()

        with uds.declare_queryable("compat/query/**", callback=reply):
            replies = list(tcp.get("compat/query/value", payload=b"request", timeout_ms=3000))
            assert len(replies) == 1 and replies[0].ok
            assert bytes(replies[0].sample.payload) == b"python-reply"
            with tcp.declare_querier("compat/query/**", timeout_ms=3000) as querier:
                replies = list(querier.get(payload=b"request"))
                assert len(replies) == 1 and replies[0].ok
                assert bytes(replies[0].sample.payload) == b"python-reply"

        callback_events = queue.Queue()
        with tcp.declare_subscriber("compat/callback/**", callback=callback_events.put):
            uds.put("compat/callback/value", b"callback")
            assert bytes(callback_events.get(timeout=10).payload) == b"callback"
        print("PASS Python TCP/UDS, native transport, metadata, deletes, queries, callbacks", flush=True)


def check_examples(directory, language, endpoint, temp):
    prefix = "z_" if language == "C" else ""
    with grpc.Session.connect(endpoint) as session:
        with session.declare_subscriber("demo/example/**") as sub:
            with process([str(directory / (prefix + "pub")), endpoint], temp / f"{language}-pub.log"):
                sample = eventually(sub.try_recv)
                assert b"hello from" in bytes(sample.payload)
        for callback in [False, True]:
            suffix = "sub_callback" if callback else "sub"
            log_path = temp / f"{language}-{suffix}.log"
            with process(["stdbuf", "-oL", str(directory / (prefix + suffix)), endpoint], log_path):
                def receive():
                    session.put("demo/example/smoke", b"sdk-smoke")
                    return True if "sdk-smoke" in log_path.read_text() else None
                eventually(receive)
        for callback in [False, True]:
            suffix = "queryable_callback" if callback else "queryable"
            with process([str(directory / (prefix + suffix)), endpoint], temp / f"{language}-{suffix}.log"):
                def get_reply():
                    replies = list(session.get("demo/query/smoke", timeout_ms=3000))
                    samples = [reply for reply in replies if reply.ok]
                    return samples if samples else None
                replies = eventually(get_reply, timeout=15)
                assert replies[0].ok and b"reply" in bytes(replies[0].sample.payload)

        def reply(query):
            query.reply(query.key_expr, b"sdk-smoke-reply", encoding="text/plain")
            query.drop()

        with session.declare_queryable("demo/query/**", callback=reply):
            for suffix in ["get", "querier"]:
                output = subprocess.run(
                    [str(directory / (prefix + suffix)), endpoint],
                    capture_output=True, text=True, timeout=15, check=True,
                ).stdout
                assert "sdk-smoke-reply" in output, output
    print(f"PASS {language} publisher, subscribers, queryables, get, querier", flush=True)


def check_restart(command, endpoint, logs):
    with contextlib.ExitStack() as resources:
        with process(command, logs / "before-restart.log"):
            session = resources.enter_context(grpc.Session.connect(endpoint))
            sub = resources.enter_context(session.declare_subscriber("compat/restart/**"))
            pub = resources.enter_context(session.declare_publisher("compat/restart/value"))
            expected = b"before-restart"

            def received():
                pub.put(expected)
                sample = sub.try_recv()
                return sample if sample is not None and bytes(sample.payload) == expected else None

            assert bytes(eventually(received).payload) == expected
        # Restart the server while keeping the same session and declarations alive.
        expected = b"after-restart"
        with process(command, logs / "after-restart.log") as server:
            assert bytes(eventually(received, timeout=20).payload) == expected
            assert server.poll() is None
            resources.close()
    print("PASS reconnect and automatic publisher/subscriber redeclaration", flush=True)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--server", type=Path, required=True)
    parser.add_argument("--plugin", type=Path)
    parser.add_argument("--c-examples", type=Path)
    parser.add_argument("--cpp-examples", type=Path)
    parser.add_argument("--logs", type=Path, default=Path("target/sdk-smoke"))
    args = parser.parse_args()
    args.logs.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="zenoh-grpc-smoke-") as directory:
        temp = Path(directory)
        grpc_port, native_port = free_port(), free_port()
        uds = temp / "grpc.sock"
        config = {
            "mode": "router",
            "listen": {"endpoints": [f"tcp/127.0.0.1:{native_port}"]},
            "scouting": {"multicast": {"enabled": False}},
            "plugins": {"grpc": {"host": "127.0.0.1", "port": grpc_port, "uds_path": str(uds)}},
        }
        if args.plugin:
            config["plugins_loading"] = {"enabled": True}
            config["plugins"]["grpc"].update({"__path__": str(args.plugin.resolve()), "__required__": True})
        config_path = temp / "config.json"
        config_path.write_text(json.dumps(config))
        server_log = args.logs / "server.log"
        with process([str(args.server.resolve()), "-c", str(config_path)], server_log) as server:
            def ready():
                if server.poll() is not None:
                    raise AssertionError(server_log.read_text())
                try:
                    with socket.create_connection(("127.0.0.1", grpc_port), timeout=0.1):
                        return True if uds.exists() else None
                except OSError:
                    return None
            try:
                eventually(ready, timeout=20)
                tcp = f"tcp://127.0.0.1:{grpc_port}"
                unix = f"unix://{uds}"
                check_python(tcp, unix, f"tcp/127.0.0.1:{native_port}")
                for path, language in [(args.c_examples, "C"), (args.cpp_examples, "C++")]:
                    if path:
                        check_examples(path.resolve(), language, unix, args.logs)
                assert server.poll() is None, server_log.read_text()
            except BaseException:
                print(server_log.read_text(), flush=True)
                raise
        check_restart([str(args.server.resolve()), "-c", str(config_path)], unix, args.logs)
    print("PASS process-level compatibility", flush=True)


if __name__ == "__main__":
    main()
