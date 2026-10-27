// Proto compilation uses protox (pure Rust) so no system `protoc` is required.
// See docs/adr/0004-protobuf-codegen-with-protox.md.

const PROTO_FILE: &str = "proto/cisa/v1/autocomplete.proto";
const PROTO_ROOT: &str = "proto";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed={PROTO_ROOT}");
    let descriptors = protox::compile([PROTO_FILE], [PROTO_ROOT])?;
    tonic_prost_build::configure()
        .build_client(true)
        .build_server(true)
        .compile_fds(descriptors)?;
    Ok(())
}
