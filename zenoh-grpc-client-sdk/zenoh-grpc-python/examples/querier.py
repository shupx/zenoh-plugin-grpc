import zenoh_grpc


with zenoh_grpc.Session.connect() as session:
    querier = session.declare_querier(
        "demo/query/**", 
        consolidation=zenoh_grpc.ConsolidationMode.MONOTONIC, 
        timeout_ms=3_000)
    # The querier will receive the replies one by one or all at once depending on the consolidation mode set by the querier (AUTO(default,LATEST), NONE(receive one by one), MONOTONIC(time monotonically consolidation), LATEST(only the latest)).

    replies = querier.get(payload=b"hahahaha", encoding="text/plain") # unblocking, returns a ReplyStream
    # the encoding is just a hint for the queryable to know how to parse the payload, it does not affect the actual payload sent to the queryable.

    for reply in replies:
        if reply.ok:
            print("sample:", reply.sample.key_expr, reply.sample.payload, reply.sample.encoding)
        else:
            print("error:", reply.error.payload, reply.error.encoding)
