use std::path::Path;

fn main() {
    let src_dir = Path::new("src");
    for entry in std::fs::read_dir(src_dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("wgsl") {
            let src = std::fs::read_to_string(&path).unwrap();
            match naga::front::wgsl::parse_str(&src) {
                Ok(_) => println!("cargo:warning=✓ WGSL ok: {}", path.display()),
                Err(e) => {
                    panic!("WGSL parse error in {}:\n{}", path.display(), e);
                }
            }
        }
    }
}
