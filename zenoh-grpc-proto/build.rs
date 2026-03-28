fn main() {
    println!("cargo:rerun-if-changed=proto/zenoh_grpc.proto");
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(&["proto/zenoh_grpc.proto"], &["proto"])
        .expect("failed to compile zenoh_grpc.proto");
}
