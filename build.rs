fn main() {
    println!("cargo:rerun-if-changed=app.rc");
    println!("cargo:rerun-if-changed=assets/icon.ico");
    println!("cargo:rerun-if-changed=assets/app.manifest");
    embed_resource::compile("app.rc", embed_resource::NONE).manifest_required().unwrap();
}
