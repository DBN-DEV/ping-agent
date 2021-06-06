fn main() {
    println!("cargo:rerun-if-changed=protos");
    tonic_build::configure()
        .build_server(false)
        .out_dir("src/grpc")
        .compile(
            &["protos/controller.proto", "protos/collector.proto"],
            &["protos"],
        )
        .unwrap();
}
