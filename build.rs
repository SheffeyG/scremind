fn main() {
    println!("cargo:rerun-if-changed=assets/app.rc");
    println!("cargo:rerun-if-changed=assets/app.ico");

    embed_resource::compile_for("assets/app.rc", ["scremind"], embed_resource::NONE);
}
