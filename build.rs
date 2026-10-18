fn main() {
    if let Ok(entries) = std::fs::read_dir("migrations") {
        for entry in entries.flatten() {
            if let Some(path) = entry.path().to_str() {
                println!("cargo:rerun-if-changed={path}");
            }
        }
    }
    println!("cargo:rerun-if-changed=migrations");
}