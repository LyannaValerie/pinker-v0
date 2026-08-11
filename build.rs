fn main() {
    println!("cargo:rerun-if-env-changed=PINKER_BUILD_COMMIT");
}
