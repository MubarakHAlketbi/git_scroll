use eframe::egui;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;

use crate::git::GitHandler;
use crate::directory::{DirectoryParser, DirectoryEntry};
use crate::visualization::{Visualizer, LayoutType, Theme};
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
    
    // UI state
    show_stats_panel: bool,
    current_layout: LayoutType,
    current_theme: Theme,
    filter_pattern: String,
    
    // Background processing channels
    clone_receiver: mpsc::Receiver<Result<PathBuf, String>>,
    parse_receiver: mpsc::Receiver<Result<DirectoryEntry, String>>,
}

impl GitScrollApp {
    /// Creates a new instance of the GitScrollApp
    ///
    /// Returns a new GitScrollApp with default values
    pub fn new() -> Self {
        // Create channels for background processing
        let (_clone_sender, clone_receiver) = mpsc::channel();
        let (_parse_sender, parse_receiver) = mpsc::channel();
        
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
            
            // UI state
            show_stats_panel: true,
            current_layout: LayoutType::Grid,
            current_theme: Theme::Light,
            filter_pattern: String::new(),
            
            // Background processing channels
            clone_receiver,
            parse_receiver,
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
            self.status_message = String::from("Invalid Git URL format");
            return;
        }
        
        // Update state
        self.is_cloning = true;
        self.status_message = String::from("Cloning repository...");
        self.ui_handler.set_loading(true);
        
        // Create channels for this operation
        let (clone_sender, clone_receiver) = mpsc::channel();
        let (parse_sender, parse_receiver) = mpsc::channel();
        self.clone_receiver = clone_receiver;
        self.parse_receiver = parse_receiver;
        
        // Update git handler with keep_repository preference
        let git_handler = GitHandler::new(self.keep_repository);
        
        // Clone the git URL for the background thread
        let git_url = self.git_url.clone();
        
        // Create a temporary directory for the repository
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
        
        // Spawn a background thread to perform the cloning and parsing
        thread::spawn(move || {
            // Clone the repository
            let repo_path_result = git_handler.clone_repository(&git_url, temp_dir.path());
            
            // Send the result back to the main thread
            let _ = clone_sender.send(repo_path_result.clone());
            
            // If cloning was successful, parse the directory structure
            if let Ok(repo_path) = repo_path_result {
                let parser = DirectoryParser::new();
                let parse_result = parser.parse_directory(&repo_path);
                let _ = parse_sender.send(parse_result);
            }
        });
    }
    
    /// Handles zoom in/out actions
    /// 
    /// # Arguments
    /// * `zoom_in` - Whether to zoom in (true) or out (false)
    fn handle_zoom(&mut self, zoom_in: bool) {
        self.visualizer.zoom(zoom_in);
    }
    
    /// Handles layout type change
    /// 
    /// # Arguments
    /// * `layout_type` - The new layout type
    fn handle_layout_change(&mut self, layout_type: LayoutType) {
        if self.current_layout != layout_type {
            self.current_layout = layout_type;
            self.visualizer.set_layout_type(layout_type);
        }
    }
    
    /// Handles theme change
    /// 
    /// # Arguments
    /// * `theme` - The new theme
    fn handle_theme_change(&mut self, theme: Theme) {
        if self.current_theme != theme {
            self.current_theme = theme.clone();
            self.visualizer.set_theme(theme);
        }
    }
    
    /// Handles filter pattern change
    /// 
    /// # Arguments
    /// * `pattern` - The new filter pattern
    fn handle_filter_change(&mut self, pattern: String) {
        if self.filter_pattern != pattern {
            self.filter_pattern = pattern.clone();
            
            // Update directory parser with the new filter
            if !pattern.is_empty() {
                self.directory_parser.add_ignore_pattern(pattern);
                
                // Re-parse the directory structure if we have a repository
                if let Some(repo_path) = &self.repository_path {
                    if let Ok(root_entry) = self.directory_parser.parse_directory(repo_path) {
                        self.directory_structure = Some(root_entry.clone());
                        self.visualizer.set_root_entry(root_entry);
                    }
                }
            }
        }
    }
    
    /// Renders the statistics panel
    /// 
    /// # Arguments
    /// * `ui` - The egui UI to render to
    fn render_stats_panel(&self, ui: &mut egui::Ui) {
        ui.heading("Repository Statistics");
        ui.add_space(10.0);
        
        if let Some(stats) = self.visualizer.directory_stats.as_ref() {
            ui.label(format!("Total Files: {}", stats.total_files));
            ui.label(format!("Total Directories: {}", stats.total_directories));
            ui.label(format!("Total Size: {} KB", stats.total_size_bytes / 1024));
            ui.label(format!("Max Depth: {}", stats.max_depth));
            
            ui.add_space(10.0);
            ui.separator();
            ui.add_space(10.0);
            
            ui.heading("File Types");
            ui.add_space(5.0);
            
            // Sort file types by count
            let mut file_types: Vec<(&String, &usize)> = stats.file_types.iter().collect();
            file_types.sort_by(|a, b| b.1.cmp(a.1));
            
            // Display top file types
            for (ext, count) in file_types.iter().take(10) {
                ui.label(format!("{}: {}", ext, count));
            }
        } else {
            ui.label("No statistics available");
        }
    }
    
    /// Renders the settings panel
    /// 
    /// # Arguments
    /// * `ui` - The egui UI to render to
    fn render_settings_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Visualization Settings");
        ui.add_space(10.0);
        
        // Layout selection
        ui.label("Layout:");
        ui.horizontal(|ui| {
            if ui.radio_value(&mut self.current_layout, LayoutType::Grid, "Grid").clicked() {
                self.handle_layout_change(LayoutType::Grid);
            }
            if ui.radio_value(&mut self.current_layout, LayoutType::Treemap, "Treemap").clicked() {
                self.handle_layout_change(LayoutType::Treemap);
            }
            if ui.radio_value(&mut self.current_layout, LayoutType::ForceDirected, "Force-Directed").clicked() {
                self.handle_layout_change(LayoutType::ForceDirected);
            }
            if ui.radio_value(&mut self.current_layout, LayoutType::Detailed, "Detailed").clicked() {
                self.handle_layout_change(LayoutType::Detailed);
            }
        });
        
        ui.add_space(10.0);
        
        // Theme selection
        ui.label("Theme:");
        ui.horizontal(|ui| {
            if ui.radio_value(&mut self.current_theme, Theme::Light, "Light").clicked() {
                self.handle_theme_change(Theme::Light);
            }
            if ui.radio_value(&mut self.current_theme, Theme::Dark, "Dark").clicked() {
                self.handle_theme_change(Theme::Dark);
            }
        });
        
        ui.add_space(10.0);
        
        // Filter pattern
        ui.label("Filter Pattern:");
        let mut pattern = self.filter_pattern.clone();
        if ui.text_edit_singleline(&mut pattern).changed() {
            self.handle_filter_change(pattern);
        }
        
        if ui.button("Apply Filter").clicked() {
            self.handle_filter_change(self.filter_pattern.clone());
        }
    }
}

impl GitScrollApp {
    // ... existing methods ...

    /// Checks for results from background operations
    fn check_background_operations(&mut self) {
        // Check for clone results
        if let Ok(repo_path_result) = self.clone_receiver.try_recv() {
            match repo_path_result {
                Ok(repo_path) => {
                    self.repository_path = Some(repo_path);
                    self.status_message = String::from("Repository cloned successfully, parsing directory...");
                },
                Err(e) => {
                    self.status_message = format!("Failed to clone repository: {}", e);
                    self.is_cloning = false;
                    self.ui_handler.set_loading(false);
                }
            }
        }
        
        // Check for parse results
        if let Ok(parse_result) = self.parse_receiver.try_recv() {
            match parse_result {
                Ok(root_entry) => {
                    // Set the directory structure
                    self.directory_structure = Some(root_entry.clone());
                    
                    // Update the visualizer
                    self.visualizer.set_root_entry(root_entry);
                    
                    // Update state
                    self.status_message = String::from("Repository parsed successfully");
                    self.is_cloning = false;
                    self.ui_handler.set_loading(false);
                },
                Err(e) => {
                    // Failed to parse directory
                    self.status_message = format!("Failed to parse repository: {}", e);
                    
                    // Clean up the repository if not keeping it
                    if !self.keep_repository && self.repository_path.is_some() {
                        let _ = self.git_handler.cleanup(self.repository_path.as_ref().unwrap());
                    }
                    
                    self.is_cloning = false;
                    self.ui_handler.set_loading(false);
                }
            }
        }
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
        
        // Check for results from background operations
        self.check_background_operations();
        
        // Left panel for settings
        egui::SidePanel::left("settings_panel")
            .resizable(true)
            .default_width(200.0)
            .show(ctx, |ui| {
                self.render_settings_panel(ui);
            });
        
        // Right panel for statistics (if enabled)
        if self.show_stats_panel && self.directory_structure.is_some() {
            egui::SidePanel::right("stats_panel")
                .resizable(true)
                .default_width(200.0)
                .show(ctx, |ui| {
                    self.render_stats_panel(ui);
                });
        }
        
        // Main central panel
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
            
            // Toggle for stats panel
            if self.directory_structure.is_some() {
                ui.checkbox(&mut self.show_stats_panel, "Show Statistics");
            }
            
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
                
                // Update animation state with current time
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs_f64();
                self.visualizer.update_animation(now);
                
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
        
        // Test valid URLs
        assert!(app.validate_git_url("https://github.com/user/repo.git"));
        assert!(app.validate_git_url("https://github.com/user/repo")); // No .git suffix is now valid
        assert!(app.validate_git_url("git@github.com:user/repo.git")); // SSH format is now valid
        assert!(app.validate_git_url("file:///path/to/repo")); // Local file path is now valid
        assert!(app.validate_git_url("/absolute/path/to/repo")); // Absolute path is now valid
        
        // Test invalid URLs
        assert!(!app.validate_git_url("invalid-url")); // No protocol or path format
    }
}