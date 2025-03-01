mod app;
mod git;
mod directory;
mod visualization;
mod ui;

use eframe::{egui, epaint};

/// Entry point for the Git Scroll application
fn main() {
    // Log startup information
    println!("Starting Git Scroll...");
    
    // Set up native options
    let native_options = eframe::NativeOptions {
        initial_window_size: Some(egui::vec2(1024.0, 768.0)),
        min_window_size: Some(egui::vec2(800.0, 600.0)),
        centered: true,
        ..Default::default()
    };
    
    // Run the application
    match eframe::run_native(
        "Git Scroll",
        native_options,
        Box::new(|cc| Box::new(app::GitScrollApp::new())),
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
