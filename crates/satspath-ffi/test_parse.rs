use uniffi_build;

fn main() {
    let result = uniffi_build::generate_scaffolding("src/satspath.udl");
    println!("Result: {:?}", result);
}
