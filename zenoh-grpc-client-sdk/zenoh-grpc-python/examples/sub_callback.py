import zenoh_grpc
import time

# with zenoh_grpc.Session.connect("unix:///tmp/zenoh-grpc.sock") as session:
# with zenoh_grpc.Session.connect("tcp://127.0.0.1:7335") as session:
with zenoh_grpc.Session.connect() as session:
    sub = session.declare_subscriber("demo/example/**", allowed_origin=1)

    def on_sample(sample):
        print("\ncallback:", sample.key_expr, sample.payload, sample.encoding)
        # print("dropped count:", sub.dropped_count())
        # time.sleep(1) # do not sleep here, just for testing the dropped count

    sub.run(on_sample)
    time.sleep(1000)
