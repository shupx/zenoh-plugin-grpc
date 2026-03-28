import zenoh_grpc
import time


with zenoh_grpc.Session.connect("tcp://127.0.0.1:7335") as session:
    pub = session.declare_publisher("demo/example/python", encoding="text/plain")
    print("pub established.")

    for i in range(5000):
        msg = f"hello from python {i}"
        pub.put(msg.encode(), encoding="text/plain")
        print("\npublished to demo/example/python:", msg)
        time.sleep(0.5)
