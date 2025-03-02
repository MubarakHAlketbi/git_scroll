mod app;
mod git;
mod directory;
mod ui;

/// Entry point for the Git Scroll application
fn main() {
    // Log startup information
    println!("Starting Git Scroll...");
    
    // Set up native options
    let native_options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1024.0, 768.0])
            .with_min_inner_size([800.0, 600.0])
            .with_position([100.0, 100.0]),
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
