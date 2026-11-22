fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rerun-if-changed=tvstudio.ico");
        println!("cargo:rerun-if-changed=tvstudio.rc");
        embed_resource::compile_for("tvstudio.rc", ["tvstudio"], embed_resource::NONE)
            .manifest_required()
            .expect("failed to embed the TV Studio icon");
    }
}
