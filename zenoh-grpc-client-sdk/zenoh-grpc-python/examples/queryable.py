import zenoh_grpc
import time

with zenoh_grpc.Session.connect() as session:
    queryable = session.declare_queryable("demo/query/**", callback=None, complete=False, allowed_origin=zenoh_grpc.Locality.ANY)

    for query in queryable.receiver():
        print("\nreceived query:", query.query_id, query.selector, query.key_expr, query.parameters, query.payload, query.encoding, query.attachment)

        query.reply(query.key_expr, b"this is a reply from queryable", encoding="text/plain")
        print("replied to query:", query.query_id)

        query.drop()
        print("dropped query")
