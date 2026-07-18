// build.rs - UniFFI scaffolding
fn main() {
    uniffi_build::generate_scaffolding("src/satspath.udl").unwrap();
}