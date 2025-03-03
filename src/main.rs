mod app;
mod git;
mod directory;
mod ui;

/// Entry point for the Git Scroll application
fn main() {
    // Log startup information
    println!("Starting Git Scroll...");
    
    // Set up native options with improved window size
    let native_options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1024.0, 768.0]) // Adjusted default size
            .with_min_inner_size([650.0, 480.0]) // Increased minimum width for better spacing
            .with_position([100.0, 100.0])
            .with_title("Git Scroll - Repository Analyzer")
            .with_icon(load_icon()),
        ..Default::default()
    };
    
    // Run the application
    match eframe::run_native(
        "Git Scroll",
        native_options,
        Box::new(|_cc| Ok(Box::new(app::GitScrollApp::new()))),
    ) {
        Ok(_) => println!("Application closed successfully"),
        Err(e) => eprintln!("Error running application: {}", e),
    }
}

/// Loads the application icon
fn load_icon() -> eframe::egui::viewport::IconData {
    // Default icon data (a simple blue square)
    let width: u32 = 32;
    let height: u32 = 32;
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    
    for y in 0..height {
        for x in 0..width {
            // Create a simple gradient icon
            let r = (x as f32 / width as f32 * 100.0) as u8;
            let g = (y as f32 / height as f32 * 100.0) as u8;
            let b = 200;
            let a = 255;
            
            rgba.push(r);
            rgba.push(g);
            rgba.push(b);
            rgba.push(a);
        }
    }
    
    eframe::egui::viewport::IconData {
        rgba,
        width,
        height,
    }
}

#[cfg(test)]
mod tests {
    // Integration tests would go here
    #[test]
    fn test_app_creation() {
        // This is a simple test to ensure the app can be created
        let app = crate::app::GitScrollApp::new();
        
        // If we got here, the app was created successfully
        assert!(true);
    }
}
