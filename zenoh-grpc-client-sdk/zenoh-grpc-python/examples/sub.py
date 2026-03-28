import zenoh_grpc
import time

with zenoh_grpc.Session.connect("tcp://127.0.0.1:7335") as session:
    with session.declare_subscriber("demo/example/**") as sub:
        for i in range(1000):
            sample = sub.recv()
            print(sample.key_expr, sample.payload, sample.encoding)
            time.sleep(0.1)
