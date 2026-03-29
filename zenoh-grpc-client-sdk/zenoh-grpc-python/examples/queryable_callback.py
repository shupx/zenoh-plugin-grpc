import zenoh_grpc
import time


with zenoh_grpc.Session.connect() as session:
    def on_query(query):
        print("\nreceived query:", query.query_id, query.selector, query.key_expr, query.parameters, query.payload, query.encoding, query.attachment)
        try:
            query.reply(query.key_expr, b"this is a reply from queryable callback", encoding="text/plain")
            print("replied to query:", query.query_id)
        finally:
            query.drop()

    queryable = session.declare_queryable("demo/query/**", callback=on_query, complete=False, allowed_origin=zenoh_grpc.Locality.ANY) 
    # this auto spawns a separate thread to handle query callbacks, so that the callback can reply to queries in time without blocking the main thread. But the callbacks all share the same thread, so they are executed sequentially. If you want to handle queries in parallel, you can set callback=None and handle the callbacks in the main thread by yourself, as shown in queryable.py.
   
    time.sleep(1000)
