use std::fs;
use std::path::Path;

fn main() {
    // Set default backend URL as compile-time environment variable
    let backend_url = "http://localhost:3000".to_string();

    // Set as cargo environment variable that can be accessed via env!() in code
    println!("cargo:rustc-env=BACKEND_URL={}", backend_url);
}
