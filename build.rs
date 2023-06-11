fn main() {
    // compile protocol buffer using protoc
    // protoc_rust_grpc::Codegen::new()
    // .out_dir("src/gen")
    // .input("./proto/raft.proto")
    // .rust_protobuf(true)
    // .run()
    // .expect("error compiling protocol buffer");
  tonic_build::compile_protos("proto/raft.proto").unwrap();

}
