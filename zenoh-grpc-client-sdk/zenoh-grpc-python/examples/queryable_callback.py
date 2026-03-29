import zenoh_grpc
import time


with zenoh_grpc.Session.connect() as session:
    def on_query(event):
        print("\nreceived query:", event.query_id, event.selector, event.key_expr, event.parameters, event.payload, event.encoding, event.attachment)
        queryable.reply(
            event.query_id,
            "demo/query/value",
            b"reply from python callback",
            encoding="text/plain",
        )
        print("replied to query:", event.query_id)

    queryable = session.declare_queryable("demo/query/**", on_query, complete=False, allowed_origin=zenoh_grpc.Locality.ANY)
   
    time.sleep(1000)
