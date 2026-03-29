import zenoh_grpc
import time

with zenoh_grpc.Session.connect() as session:
    queryable = session.declare_queryable("demo/query/**", callback=None, complete=False, allowed_origin=zenoh_grpc.Locality.ANY)

    event = queryable.recv()
    print("\nreceived query:", event.query_id, event.selector, event.key_expr, event.parameters, event.payload, event.encoding, event.attachment)
    
    queryable.reply(
        event.query_id, "demo/query/value", b"reply from python", encoding="text/plain"
    )
    print("replied to query:", event.query_id)
    time.sleep(20)
