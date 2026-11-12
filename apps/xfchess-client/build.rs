use std::fs;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = std::env::var("OUT_DIR")?;
    let out_dir = Path::new(&out_dir);
    let target_dir = out_dir
        .ancestors()
        .nth(3)
        .ok_or("OUT_DIR does not contain a Cargo target directory")?;
    let assets_dest = target_dir.join("assets");

    let assets_src = Path::new("assets");
    if assets_src.exists() {
        println!("cargo:rerun-if-changed=assets");
        if let Err(e) = copy_dir_recursive(assets_src, &assets_dest) {
            eprintln!("Warning: Failed to copy assets: {}", e);
        }
    }
    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    if !dst.exists() {
        fs::create_dir_all(dst)?;
    }
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}
