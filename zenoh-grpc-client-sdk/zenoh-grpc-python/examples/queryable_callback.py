import zenoh_grpc
import time


with zenoh_grpc.Session.connect() as session:
    def on_query(query):
        print("\nreceived query:", query.query_id, query.selector, query.key_expr, query.parameters, query.payload, query.encoding, query.attachment)
        try:
            # You can reply multiple times to the same query by calling reply() multiple times. The querier will receive the replies one by one or all at once depending on the consolidation mode set by the querier (AUTO(default,LATEST), NONE(receive one by one), MONOTONIC(monotonically consolidation), LATEST(only the latest)).
            query.reply(query.key_expr, b"this is a reply1 from queryable callback", encoding="text/plain")
            print("reply1 to query:", query.query_id)

            time.sleep(1) # simulate some processing time, not recommended to sleep in the callback as it will block the callback thread and delay the processing of other queries.

            query.reply(query.key_expr, b"this is a reply2 from queryable callback", encoding="text/plain")
            print("reply2 to query:", query.query_id)
        finally:
            # Notice: you should explicitly drop() after you finished replying to send ResponseFinal message to querier, otherwise the querier will wait until timeout and will not receive the replies until then.
            query.drop()

    queryable = session.declare_queryable("demo/query/**", callback=on_query, complete=False, allowed_origin=zenoh_grpc.Locality.ANY) 
    # this auto spawns a separate thread to handle query callbacks, so that the callback can reply to queries in time without blocking the main thread. But the callbacks all share the same thread, so they are executed sequentially. 
   
    time.sleep(1000)
