use eframe::egui;
use std::path::PathBuf;

use crate::git::GitHandler;
use crate::directory::{DirectoryParser, DirectoryEntry};
use crate::visualization::Visualizer;
use crate::ui::UiHandler;

/// Main application state for Git Scroll
pub struct GitScrollApp {
    // Input state
    git_url: String,
    keep_repository: bool,
    
    // Application state
    status_message: String,
    is_cloning: bool,
    
    // Repository data
    repository_path: Option<PathBuf>,
    directory_structure: Option<DirectoryEntry>,
    
    // Module handlers
    git_handler: GitHandler,
    directory_parser: DirectoryParser,
    visualizer: Visualizer,
    ui_handler: UiHandler,
}

impl GitScrollApp {
    /// Creates a new instance of the GitScrollApp
    /// 
    /// Returns a new GitScrollApp with default values
    pub fn new() -> Self {
        // Initialize with default values
        Self {
            git_url: String::new(),
            keep_repository: false,
            status_message: String::from("Ready"),
            is_cloning: false,
            repository_path: None,
            directory_structure: None,
            
            // Initialize module handlers
            git_handler: GitHandler::new(false),
            directory_parser: DirectoryParser::new(),
            visualizer: Visualizer::new(),
            ui_handler: UiHandler::new(),
        }
    }
    
    /// Validates the Git URL format
    /// 
    /// # Arguments
    /// * `url` - The Git URL to validate
    /// 
    /// # Returns
    /// * `bool` - True if the URL is valid, false otherwise
    fn validate_git_url(&self, url: &str) -> bool {
        GitHandler::validate_url(url)
    }
    
    /// Handles the clone button click
    ///
    /// Initiates the repository cloning process if the URL is valid
    fn handle_clone_button(&mut self) {
        if self.is_cloning {
            return; // Already cloning
        }
        
        if !self.validate_git_url(&self.git_url) {
            self.status_message = String::from("Invalid Git URL. Must be HTTPS and end with .git");
            return;
        }
        
        // Update state
        self.is_cloning = true;
        self.status_message = String::from("Cloning repository...");
        self.ui_handler.set_loading(true);
        
        // Update git handler with keep_repository preference
        self.git_handler = GitHandler::new(self.keep_repository);
        
        // Create a temporary directory for the repository
        // If keep_repository is true, we'll use a more permanent location later
        let temp_dir = match tempfile::Builder::new()
            .prefix("git_scroll_")
            .tempdir() {
                Ok(dir) => dir,
                Err(e) => {
                    self.status_message = format!("Failed to create temporary directory: {}", e);
                    self.is_cloning = false;
                    self.ui_handler.set_loading(false);
                    return;
                }
            };
        
        // Clone the repository
        match self.git_handler.clone_repository(&self.git_url, temp_dir.path()) {
            Ok(repo_path) => {
                // Clone successful, parse the directory structure
                match self.directory_parser.parse_directory(&repo_path) {
                    Ok(root_entry) => {
                        // Store the repository path
                        self.repository_path = Some(repo_path);
                        
                        // Set the directory structure
                        self.directory_structure = Some(root_entry.clone());
                        
                        // Update the visualizer
                        self.visualizer.set_root_entry(root_entry);
                        
                        // Update state
                        self.status_message = String::from("Repository cloned successfully");
                    },
                    Err(e) => {
                        // Failed to parse directory
                        self.status_message = format!("Failed to parse repository: {}", e);
                        
                        // Clean up the repository if not keeping it
                        if !self.keep_repository {
                            let _ = self.git_handler.cleanup(&repo_path);
                        }
                    }
                }
            },
            Err(e) => {
                // Clone failed
                self.status_message = format!("Failed to clone repository: {}", e);
            }
        }
        
        // Update state
        self.is_cloning = false;
        self.ui_handler.set_loading(false);
    }
    
    /// Handles zoom in/out actions
    /// 
    /// # Arguments
    /// * `zoom_in` - Whether to zoom in (true) or out (false)
    fn handle_zoom(&mut self, zoom_in: bool) {
        self.visualizer.zoom(zoom_in);
    }
}

impl eframe::App for GitScrollApp {
    /// Updates the application state and renders the UI
    /// 
    /// # Arguments
    /// * `ctx` - The egui context
    /// * `_frame` - The eframe frame
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Apply custom styling
        crate::ui::style::apply_style(ctx);
        
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Git Scroll");
            
            // Top section: URL input and clone button
            let clone_clicked = self.ui_handler.render_top_bar(
                ui,
                &mut self.git_url,
                &mut self.keep_repository,
            );
            
            if clone_clicked {
                self.handle_clone_button();
            }
            
            // Status message
            self.ui_handler.render_status_bar(ui, &self.status_message);
            
            // Main content area
            ui.add_space(10.0);
            ui.separator();
            ui.add_space(10.0);
            
            if self.directory_structure.is_none() {
                self.ui_handler.render_empty_state(ui);
            } else {
                // Zoom controls
                let (zoom_in, zoom_out) = self.ui_handler.render_zoom_controls(ui);
                
                if zoom_in {
                    self.handle_zoom(true);
                }
                
                if zoom_out {
                    self.handle_zoom(false);
                }
                
                // Visualization
                let available_height = ui.available_height() - 20.0;
                ui.allocate_rect(
                    egui::Rect::from_min_size(
                        ui.min_rect().min,
                        egui::vec2(ui.available_width(), available_height)
                    ),
                    egui::Sense::click_and_drag()
                );
                
                // Handle mouse interaction
                let pointer_pos = ctx.pointer_hover_pos();
                let clicked = ctx.input(|i| i.pointer.primary_clicked());
                self.visualizer.handle_interaction(ui, pointer_pos, clicked);
                
                // Render visualization
                self.visualizer.render(ui);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_validate_git_url() {
        // Create a new app instance for testing
        let app = GitScrollApp::new();
        
        // Test valid URL
        assert!(app.validate_git_url("https://github.com/user/repo.git"));
        
        // Test invalid URLs
        assert!(!app.validate_git_url("http://github.com/user/repo.git")); // Not HTTPS
        assert!(!app.validate_git_url("https://github.com/user/repo")); // No .git suffix
        assert!(!app.validate_git_url("git@github.com:user/repo.git")); // SSH format
    }
}