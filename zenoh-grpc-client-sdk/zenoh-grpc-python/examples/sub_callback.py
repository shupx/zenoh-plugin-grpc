import zenoh_grpc
import time


def on_sample(event):
    print("callback:", event.key_expr, event.payload)


with zenoh_grpc.Session.connect("tcp://127.0.0.1:7335") as session:
    sub = session.declare_subscriber("demo/example/**")
    sub.run(on_sample)
    time.sleep(1000)
