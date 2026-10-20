fn main() {
    println!("cargo:rerun-if-changed=src/compiler/v1/grammar.pest");
    println!("cargo:rerun-if-changed=src/compiler/v2/grammar.pest");
}
