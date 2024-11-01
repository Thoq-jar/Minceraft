use std::env;
use chrono::Utc;
use std::fs;
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("built_info.rs");

    let build_info = format!(
        r#"
        pub const BUILD_TIMESTAMP: &str = "{}";
        pub const BUILD_VERSION: &str = "{}";
        "#,
        Utc::now(),
        env!("CARGO_PKG_VERSION")
    );

    fs::write(dest_path, build_info).unwrap();
} 