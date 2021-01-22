fn main() {
    println!("cargo:rerun-if-changed=protos");
    tonic_build::configure()
        .build_server(false)
        .out_dir("src/reporter")
        .compile(&["protos/collector.proto"], &["protos"])
        .unwrap();
    tonic_build::configure()
        .build_server(false)
        .out_dir("src/commander")
        .compile(&["protos/controller.proto"], &["protos"])
        .unwrap();
}
