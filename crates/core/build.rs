fn main() {
    println!("cargo:rerun-if-changed=../../schema/session_checkpoint.capnp");
    ::capnpc::CompilerCommand::new()
        .file("../../schema/session_checkpoint.capnp")
        .src_prefix("../../schema")
        .run()
        .expect("Failed to compile session_checkpoint.capnp schema");
}
