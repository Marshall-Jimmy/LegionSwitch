fn main() {
    println!("cargo:rerun-if-changed=resources/app.rc");
    println!("cargo:rerun-if-changed=resources/app-preview.rc");
    println!("cargo:rerun-if-changed=resources/app.manifest");
    println!("cargo:rerun-if-changed=resources/app-preview.manifest");
    println!("cargo:rerun-if-changed=resources/legion-switch.ico");
    println!("cargo:rerun-if-changed=resources/quiet.ico");
    println!("cargo:rerun-if-changed=resources/balanced.ico");
    println!("cargo:rerun-if-changed=resources/beast.ico");
    println!("cargo:rerun-if-changed=resources/mode-quiet.ico");
    println!("cargo:rerun-if-changed=resources/mode-balanced.ico");
    println!("cargo:rerun-if-changed=resources/mode-beast.ico");
    println!("cargo:rerun-if-env-changed=LEGIONSWITCH_PREVIEW");

    let rc = if std::env::var_os("LEGIONSWITCH_PREVIEW").is_some() {
        "resources/app-preview.rc"
    } else {
        "resources/app.rc"
    };
    let _ = embed_resource::compile(rc, embed_resource::NONE);
}
