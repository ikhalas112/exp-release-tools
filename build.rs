fn main() {
    for key in [
        "R2_ACCESS_KEY_ID",
        "R2_SECRET_ACCESS_KEY",
        "R2_BUCKET_NAME",
        "R2_ENDPOINT",
        "MAXION_BUILD_SECRET",
    ] {
        let val = std::env::var(key).unwrap_or_default();
        println!("cargo:rustc-env=EMBED_{key}={val}");
    }
}
