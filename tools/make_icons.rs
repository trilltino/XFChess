use std::path::Path;

fn main() {
    // Load the zebra logo
    let img = image::open("zebra_logo.png").expect("Failed to load zebra_logo.png");
    
    // Resize to various sizes
    let img_256 = img.resize(256, 256, image::imageops::FilterType::Lanczos3);
    let img_128 = img.resize(128, 128, image::imageops::FilterType::Lanczos3);
    let img_32 = img.resize(32, 32, image::imageops::FilterType::Lanczos3);
    
    // Save PNG icons
    img_256.save("tauri/icons/icon.png").expect("Failed to save icon.png");
    img_128.save("tauri/icons/128x128.png").expect("Failed to save 128x128.png");
    img_32.save("tauri/icons/32x32.png").expect("Failed to save 32x32.png");
    
    // For ICO, we need to create a multi-icon file
    // For now, just save the 256x256 as the main icon
    img_256.save("tauri/icons/icon.ico").expect("Failed to save icon.ico");
    
    println!("Icons created successfully!");
}
