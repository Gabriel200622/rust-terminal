fn main() -> std::io::Result<()> {
    println!("cargo:rerun-if-changed=assets/icons/neptune.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new()
            .set_icon("assets/icons/neptune.ico")
            .set("ProductName", "Neptune")
            .set("FileDescription", "Neptune terminal")
            .compile()?;
    }
    Ok(())
}
