import zenoh_grpc
import time

# with zenoh_grpc.Session.connect("unix:///tmp/zenoh-grpc.sock") as session:
# with zenoh_grpc.Session.connect("tcp://127.0.0.1:7335") as session:
with zenoh_grpc.Session.connect() as session:
    pub = session.declare_publisher("demo/example/python", encoding="text/plain")
    print("pub established.")

    for i in range(5000000000):
        msg = f"hello from python {i}"
        pub.put(msg.encode(), encoding="text/plain") # non-blocking, will drop if the internal queue is full
        print("\npublished to demo/example/python:", msg)
        print("publish queue dropped count:", pub.send_dropped_count()) # print the number of dropped messages
        time.sleep(0.01) # gRPC server can only handle 10_000Hz for all clients with no dropped messages on my machine. So do not send messages too fast but send in batch and low frequency, or you will see the dropped count increasing. You can also adjust the sleep time to see how it affects the dropped count.
