import zenoh_grpc
import time

with zenoh_grpc.Session.connect() as session:
    with session.declare_subscriber("demo/example/**") as sub:
        for i in range(1000):
            sample = sub.recv() 
            # not recommended to use recv() in a loop, as it is blocking and will block the main thread, and due to the inner rust implementation, it can not be killed by Ctrl+C. It is better to use a callback to receive samples, as shown in sub_callback.py. But here we just want to show how to use recv() in a loop, so we use it here for simplicity.
            print(sample.key_expr, sample.payload, sample.encoding)
            time.sleep(0.1)
