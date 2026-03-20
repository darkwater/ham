fn main() {
    println!("cargo:rerun-if-changed=ham-server/migrations");
}
