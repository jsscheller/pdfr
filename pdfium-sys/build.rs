use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rustc-link-lib=pdfium");
    println!("cargo:rerun-if-changed=wrapper.h");

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());

    bindgen::Builder::default()
        .header("wrapper.h")
        // TODO https://github.com/rust-lang/rust-bindgen/issues/751
        .clang_arg("-fvisibility=default")
        .opaque_type("std::.*")
        .whitelist_type("FPDF.*")
        .whitelist_function("FPDF.*")
        .whitelist_var("FPDF.*")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        .generate()
        .unwrap()
        .write_to_file(out_dir.join("bindings.rs"))
        .unwrap();
}
