use std::env;
use std::process::Command;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();

    Command::new("cp")
        .arg("-r")
        .arg("liburing")
        .arg(out_dir.clone())
        .status()
        .expect("copy liburing to out_dir");
    Command::new("make")
        .arg("j4")
        .current_dir(format!("{}/liburing", out_dir.clone()))
        .status()
        .expect("failed to build liburing.a");

    println!("cargo:rerun-if-changed=wrapper.h");
    println!("cargo:rustc-link-lib=static=uring-ffi");
    println!(
        "cargo:rustc-link-search=native={}/liburing/src",
        out_dir.clone()
    );

    // Generate bindings
    let bindings = bindgen::Builder::default()
        // .allowlist_function("__io_uring.*")
        // .allowlist_function("io_uring.*")
        // .allowlist_var("IORING.*")
        // .allowlist_var("IOSQE.*")
        // .allowlist_item("IORING.*")
        // .allowlist_item("IOSQE.*")
        // .allowlist_type("io_uring.*")
        .header("wrapper.h")
        .generate()
        .expect("Unable to generate bindings");

    // Write the bindings to the $OUT_DIR/bindings.rs file.
    // let out_path = PathBuf::from(out_dir);
    bindings
        .write_to_file("src/bindings.rs")
        .expect("Couldn't write bindings!");
}
