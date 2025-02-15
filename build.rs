use std::io::Result;
fn main() -> Result<()> {
    prost_build::compile_protos(
        &[
            "src/ExtensionIdentityProto.proto",
            "src/ExtensionRequestProto.proto",
            "src/ExtensionIdentityProto.proto",
        ],
        &["src/"],
    )?;
    Ok(())
}
