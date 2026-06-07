fn main() {
    // Tell cargo that stub_compiled is a valid cfg condition
    println!("cargo:rustc-check-cfg=cfg(stub_compiled)");
}
