import threading

import zenoh_grpc

done = threading.Event()


with zenoh_grpc.Session.connect() as session:
    queryable = session.declare_queryable("demo/query/**", complete=False)

    def on_query(event):
        queryable.reply(
            event.query_id,
            "demo/query/value",
            b"reply from python callback",
            encoding="text/plain",
        )
        done.set()

    queryable.run(on_query)
    if not done.wait(timeout=10):
        raise TimeoutError("timed out waiting for a query")
