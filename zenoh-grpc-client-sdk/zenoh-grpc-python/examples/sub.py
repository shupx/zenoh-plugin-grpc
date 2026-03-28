import zenoh_grpc


with zenoh_grpc.Session.connect("tcp://127.0.0.1:7335") as session:
    with session.declare_subscriber("demo/example/**") as sub:
        event = sub.recv()
        print(event.key_expr, event.payload)
