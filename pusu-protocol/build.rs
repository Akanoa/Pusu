fn main() -> Result<(), Box<dyn std::error::Error>> {
    prost_build::compile_protos(&["src/protocol.proto", "src/channel.proto"], &["src"])?;
    Ok(())
}
