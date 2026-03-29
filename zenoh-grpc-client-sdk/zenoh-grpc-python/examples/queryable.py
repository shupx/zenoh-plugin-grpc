import zenoh_grpc
import time

with zenoh_grpc.Session.connect() as session:
    queryable = session.declare_queryable("demo/query/**", callback=None, complete=False, allowed_origin=zenoh_grpc.Locality.ANY)

    while True:
        # not recommended to use recv() in a loop, as it is blocking and will block the main thread, and due to the inner rust implementation, it can not be killed by Ctrl+C. It is better to use a callback to receive samples, as shown in sub_callback.py. But here we just want to show how to use recv() in a loop, so we use it here for simplicity.
        query = queryable.receiver().recv() # blocking, returns a Query object when a query is received
        print("\nreceived query:", query.query_id, query.selector, query.key_expr, query.parameters, query.payload, query.encoding, query.attachment)

        # You can reply multiple times to the same query by calling reply() multiple times. The querier will receive the replies one by one or all at once depending on the consolidation mode set by the querier (AUTO(default,LATEST), NONE(receive one by one), MONOTONIC(monotonically consolidation), LATEST(only the latest)).

        query.reply(query.key_expr, b"this is a reply1 from queryable", encoding="text/plain")
        print("reply 1 to query:", query.query_id)

        time.sleep(1) # simulate some processing time

        query.reply(query.key_expr, b"this is a reply2 from queryable", encoding="text/plain")
        print("reply 2 to query:", query.query_id)

        # Notice: you should explicitly drop() after you finished replying to send ResponseFinal message to querier, otherwise the querier will wait until timeout and will not receive the replies until then.
        query.drop()
        print("dropped query")
