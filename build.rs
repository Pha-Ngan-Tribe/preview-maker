fn main() {
    println!("cargo:rerun-if-changed=windows/AppIcon.ico");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("windows/AppIcon.ico");
        resource
            .compile()
            .expect("compile Windows application resources");
    }
}
