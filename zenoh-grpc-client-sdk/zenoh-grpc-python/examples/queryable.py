import zenoh_grpc


with zenoh_grpc.Session.connect() as session:
    with session.declare_queryable("demo/query/**", complete=False) as queryable:
        event = queryable.recv()
        queryable.reply(
            event.query_id, "demo/query/value", b"reply from python", encoding="text/plain"
        )
