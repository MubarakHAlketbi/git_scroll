use eframe::egui;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use eframe::epaint::{Margin, CornerRadius};
use egui::LayerId;

use crate::git::GitHandler;
use crate::directory::{DirectoryParser, DirectoryEntry};
use crate::ui::UiHandler;

/// Represents a file's metadata for the list view
#[derive(Clone)]
pub struct FileInfo {
    pub index: usize,          // Order in the list
    pub path: PathBuf,         // Full path to the file
    pub tokens: usize,         // Number of tokens in the file
    pub selected: bool,        // Whether the file is selected
}

/// Counts tokens in a file by splitting on whitespace
fn count_tokens(path: &Path) -> usize {
    // Define text file extensions
    let text_extensions = [
        "txt", "rs", "py", "js", "md", "html", "css", "json", "yaml", "toml",
    ];

    // Check if the file has a text extension
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        if !text_extensions.contains(&ext.to_lowercase().as_str()) {
            return 0; // Skip non-text files
        }
    } else {
        return 0; // No extension, assume binary
    }

    // Read file content and count words
    match std::fs::read_to_string(path) {
        Ok(content) => content.split_whitespace().count(),
        Err(_) => 0, // Return 0 if file can't be read
    }
}

/// Enum for sortable columns
#[derive(PartialEq)]
pub enum SortColumn {
    Index,
    Name,
    Tokens,
}

/// Enum for sort direction
#[derive(PartialEq)]
pub enum SortDirection {
    Ascending,
    Descending,
}

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
    ui_handler: UiHandler,
    
    // UI state
    show_stats_panel: bool,
    filter_pattern: String,
    
    // File list state
    file_list: Vec<FileInfo>,
    sort_column: SortColumn,
    sort_direction: SortDirection,
    is_loading_tokens: bool,
    
    // Table UI state
    column_widths: [f32; 3], // Widths for Index, Name, Tokens columns
    
    // Background processing channels
    clone_receiver: mpsc::Receiver<Result<PathBuf, String>>,
    parse_receiver: mpsc::Receiver<Result<DirectoryEntry, String>>,
    token_receiver: mpsc::Receiver<(usize, PathBuf, usize)>,
}

impl GitScrollApp {
    /// Creates a new instance of the GitScrollApp
    ///
    /// Returns a new GitScrollApp with default values
    pub fn new() -> Self {
        // Create channels for background processing
        let (_clone_sender, clone_receiver) = mpsc::channel();
        let (_parse_sender, parse_receiver) = mpsc::channel();
        let (_token_sender, token_receiver) = mpsc::channel();
        
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
            ui_handler: UiHandler::new(),
            
            // UI state
            show_stats_panel: true,
            filter_pattern: String::new(),
            
            // File list state
            file_list: Vec::new(),
            sort_column: SortColumn::Index,
            sort_direction: SortDirection::Ascending,
            is_loading_tokens: false,
            
            // Table UI state
            column_widths: [60.0, 400.0, 100.0], // Default widths for columns
            
            // Background processing channels
            clone_receiver,
            parse_receiver,
            token_receiver,
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
    
    // Square-related methods removed (handle_zoom, handle_layout_change, handle_theme_change)
    
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
                        
                        // Refresh the file list with updated filters
                        self.populate_file_list(&root_entry);
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
        
        if !self.file_list.is_empty() {
            let total_files = self.file_list.len();
            let total_tokens = self.file_list.iter().map(|f| f.tokens).sum::<usize>();
            let avg_tokens = if total_files > 0 { total_tokens / total_files } else { 0 };
            
            ui.label(format!("Total Text Files: {}", total_files));
            ui.label(format!("Total Tokens: {}", total_tokens));
            ui.label(format!("Average Tokens per File: {}", avg_tokens));
            
            ui.add_space(10.0);
            ui.separator();
            ui.add_space(10.0);
            
            ui.heading("Top Files by Token Count");
            ui.add_space(5.0);
            
            // Get top files by token count
            let mut top_files = self.file_list.clone();
            top_files.sort_by(|a, b| b.tokens.cmp(&a.tokens));
            
            // Display top files
            for file in top_files.iter().take(10) {
                let file_name = file.path.file_name()
                    .unwrap_or_default()
                    .to_string_lossy();
                ui.label(format!("{}: {} tokens", file_name, file.tokens));
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
        ui.heading("File List Settings");
        ui.add_space(10.0);
        
        // Sort settings
        ui.label("Sort By:");
        ui.horizontal(|ui| {
            if ui.radio_value(&mut self.sort_column, SortColumn::Index, "Number").clicked() {
                self.sort_file_list();
            }
            if ui.radio_value(&mut self.sort_column, SortColumn::Name, "Name").clicked() {
                self.sort_file_list();
            }
            if ui.radio_value(&mut self.sort_column, SortColumn::Tokens, "Tokens").clicked() {
                self.sort_file_list();
            }
        });
        
        ui.add_space(5.0);
        
        // Sort direction
        ui.label("Sort Direction:");
        ui.horizontal(|ui| {
            if ui.radio_value(&mut self.sort_direction, SortDirection::Ascending, "Ascending").clicked() {
                self.sort_file_list();
            }
            if ui.radio_value(&mut self.sort_direction, SortDirection::Descending, "Descending").clicked() {
                self.sort_file_list();
            }
        });
        
        ui.add_space(10.0);
        
        // Theme selection removed - no longer needed
        
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
    /// Shows an error dialog with the given message
    ///
    /// # Arguments
    /// * `ctx` - The egui context
    /// * `error_message` - The error message to display
    fn show_error_dialog(&self, ctx: &egui::Context, error_message: &str) {
        egui::Window::new("Error")
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(error_message);
                if ui.button("OK").clicked() {
                    // Dialog will close automatically when button is clicked
                }
            });
    }

    /// Populates the file list from the directory structure
    fn populate_file_list(&mut self, root_entry: &DirectoryEntry) {
        self.file_list.clear();
        let files = self.directory_parser.get_all_files(root_entry);
        
        if files.is_empty() {
            return; // No files to process
        }
        
        // Create a new channel for this operation
        let (token_sender, token_receiver) = mpsc::channel();
        self.token_receiver = token_receiver;
        self.is_loading_tokens = true;
        
        // Create a placeholder for each file with 0 tokens initially
        for (index, path) in files.iter().enumerate() {
            self.file_list.push(FileInfo {
                index,
                path: path.clone(),
                tokens: 0, // Will be updated asynchronously
                selected: false, // Not selected by default
            });
        }
        
        // Spawn threads to count tokens for each file
        for (index, path) in files.into_iter().enumerate() {
            let sender = token_sender.clone();
            let path_clone = path.clone();
            
            thread::spawn(move || {
                let tokens = count_tokens(&path_clone);
                let _ = sender.send((index, path_clone, tokens));
            });
        }
        
        // Initial sort (will be updated as tokens are counted)
        self.sort_file_list();
    }

    /// Sorts the file list based on current sort settings
    fn sort_file_list(&mut self) {
        match self.sort_column {
            SortColumn::Index => {
                self.file_list.sort_by(|a, b| {
                    match self.sort_direction {
                        SortDirection::Ascending => a.index.cmp(&b.index),
                        SortDirection::Descending => b.index.cmp(&a.index),
                    }
                });
            }
            SortColumn::Name => {
                self.file_list.sort_by(|a, b| {
                    let a_name = a.path.file_name().unwrap_or_default().to_string_lossy();
                    let b_name = b.path.file_name().unwrap_or_default().to_string_lossy();
                    match self.sort_direction {
                        SortDirection::Ascending => a_name.cmp(&b_name),
                        SortDirection::Descending => b_name.cmp(&a_name),
                    }
                });
            }
            SortColumn::Tokens => {
                self.file_list.sort_by(|a, b| {
                    match self.sort_direction {
                        SortDirection::Ascending => a.tokens.cmp(&b.tokens),
                        SortDirection::Descending => b.tokens.cmp(&a.tokens),
                    }
                });
            }
        }
    }

    /// Checks for results from background operations
    ///
    /// # Arguments
    /// * `ctx` - The egui context
    fn check_background_operations(&mut self, ctx: &egui::Context) {
        // Check for clone results
        if let Ok(repo_path_result) = self.clone_receiver.try_recv() {
            match repo_path_result {
                Ok(repo_path) => {
                    self.repository_path = Some(repo_path);
                    self.status_message = String::from("Repository cloned successfully, parsing directory...");
                },
                Err(e) => {
                    let error_message = format!("Failed to clone repository: {}", e);
                    self.status_message = error_message.clone();
                    self.is_cloning = false;
                    self.ui_handler.set_loading(false);
                    
                    // Show error dialog for critical errors
                    self.show_error_dialog(ctx, &error_message);
                }
            }
        }
        
        // Check for parse results
        if let Ok(parse_result) = self.parse_receiver.try_recv() {
            match parse_result {
                Ok(root_entry) => {
                    // Set the directory structure
                    self.directory_structure = Some(root_entry.clone());
                    
                    // Populate file list
                    self.populate_file_list(&root_entry);
                    
                    // Update state
                    self.status_message = String::from("Repository parsed successfully");
                    self.is_cloning = false;
                    self.ui_handler.set_loading(false);
                },
                Err(e) => {
                    // Failed to parse directory
                    let error_message = format!("Failed to parse repository: {}", e);
                    self.status_message = error_message.clone();
                    
                    // Clean up the repository if not keeping it
                    if !self.keep_repository && self.repository_path.is_some() {
                        let _ = self.git_handler.cleanup(self.repository_path.as_ref().unwrap());
                    }
                    
                    self.is_cloning = false;
                    self.ui_handler.set_loading(false);
                    
                    // Show error dialog for critical errors
                    self.show_error_dialog(ctx, &error_message);
                }
            }
        }
        
        // Check for token counting results
        if self.is_loading_tokens {
            let mut received_count = 0;
            let mut all_received = false;
            
            // Try to receive as many token results as possible without blocking
            while let Ok((index, path, tokens)) = self.token_receiver.try_recv() {
                received_count += 1;
                
                // Update the token count for the file at the given index
                if index < self.file_list.len() {
                    // Find the file with the matching index and path
                    for file in &mut self.file_list {
                        if file.index == index && file.path == path {
                            file.tokens = tokens;
                            break;
                        }
                    }
                }
                
                // Check if we've received all results (assuming file_list is populated)
                if received_count >= self.file_list.len() {
                    all_received = true;
                    break;
                }
            }
            
            // If we received any results, resort the list
            if received_count > 0 {
                self.sort_file_list();
            }
            
            // If all results are received, update the loading state
            if all_received {
                self.is_loading_tokens = false;
                self.status_message = String::from("Token counting completed");
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
        // Apply custom styling based on dark mode setting
        crate::ui::style::apply_style(ctx, self.ui_handler.is_dark_mode());
        
        // Check for results from background operations
        self.check_background_operations(ctx);
        
        // Top panel for URL input and controls
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.add_space(4.0);
            
            // Top bar with URL input and clone button
            let (clone_clicked, clear_clicked) = self.ui_handler.render_top_bar(
                ui,
                &mut self.git_url,
                &mut self.keep_repository,
            );
            
            if clone_clicked {
                self.handle_clone_button();
            }
            
            if clear_clicked {
                self.clear_repository();
            }
            
            ui.add_space(4.0);
        });
        
        // Controls panel for sorting and filtering
        if self.directory_structure.is_some() {
            egui::TopBottomPanel::top("controls_panel").show(ctx, |ui| {
                ui.add_space(4.0);
                
                // Controls bar with sorting and filtering options
                if self.ui_handler.render_controls_bar(
                    ui,
                    &mut self.sort_column,
                    &mut self.sort_direction,
                    &mut self.filter_pattern,
                ) {
                    // If controls changed, update the file list
                    self.handle_filter_change(self.filter_pattern.clone());
                    self.sort_file_list();
                }
                
                ui.add_space(4.0);
            });
        }
        
        // Bottom panel for status and stats
        egui::TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
            ui.add_space(4.0);
            
            // Status bar
            self.ui_handler.render_status_bar(ui, &self.status_message, self.is_loading_tokens);
            
            // Stats bar (if repository is loaded)
            if self.directory_structure.is_some() {
                ui.add_space(4.0);
                ui.separator();
                ui.add_space(4.0);
                self.ui_handler.render_stats_bar(ui, &self.file_list);
            }
            
            ui.add_space(4.0);
        });
        
        // Main central panel
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.directory_structure.is_none() {
                // Empty state when no repository is loaded
                self.ui_handler.render_empty_state(ui, &mut self.git_url);
            } else {
                // Show file list heading
                ui.heading("Repository Files");
                
                ui.add_space(8.0);
                
                // Calculate max tokens for color scaling
                let max_tokens = self.file_list.iter()
                    .map(|f| f.tokens)
                    .max()
                    .unwrap_or(1);
                
                // Get row colors for striping
                let (even_row_color, odd_row_color) =
                    crate::ui::style::row_colors(self.ui_handler.is_dark_mode());
                
                // Get header color
                let header_color = crate::ui::style::header_color(self.ui_handler.is_dark_mode());
                
                // File list table
                egui::ScrollArea::vertical().show(ui, |ui| {
                    // Table header with custom styling
                    let header_frame = egui::Frame::default()
                        .fill(header_color)
                        .inner_margin(Margin::symmetric(8, 4));
                    
                    header_frame.show(ui, |ui| {
                        egui::Grid::new("file_list_header")
                            .num_columns(3)
                            .spacing([8.0, 4.0])
                            .show(ui, |ui| {
                                let headers = [
                                    ("Number", SortColumn::Index, self.column_widths[0]),
                                    ("File Name", SortColumn::Name, self.column_widths[1]),
                                    ("Token Count", SortColumn::Tokens, self.column_widths[2]),
                                ];
                                
                                for (i, (text, col, width)) in headers.iter().enumerate() {
                                    let sort_indicator = if self.sort_column == *col {
                                        if self.sort_direction == SortDirection::Ascending { "↑" } else { "↓" }
                                    } else { "" };
                                    
                                    let header_text = format!("{} {}", text, sort_indicator);
                                    ui.add_sized([*width, 0.0], egui::Label::new(egui::RichText::new(header_text).strong()));
                                    
                                    // Add resize handle between columns
                                    if i < 2 { // Only between columns
                                        // Create a small draggable area for resizing
                                        let resize_id = ui.id().with(("resize", i));
                                        let resize_rect = egui::Rect::from_min_size(
                                            ui.cursor().min,
                                            egui::vec2(4.0, 20.0)
                                        );
                                        
                                        let resize_response = ui.interact(
                                            resize_rect,
                                            resize_id,
                                            egui::Sense::drag()
                                        );
                                        
                                        if resize_response.dragged() {
                                            let delta = ui.input(|i| i.pointer.delta().x);
                                            self.column_widths[i] += delta;
                                            self.column_widths[i] = self.column_widths[i].clamp(50.0, 600.0);
                                        }
                                        
                                        // Show resize cursor on hover
                                        if resize_response.hovered() {
                                            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                                        }
                                    }
                                }
                                
                                ui.end_row();
                            });
                    });
                    
                    // Table body
                    egui::Grid::new("file_list_grid")
                        .num_columns(3)
                        .spacing([8.0, 4.0])
                        .striped(false) // We'll handle striping manually
                        .show(ui, |ui| {
                            // File rows
                            for (i, file) in self.file_list.iter().enumerate() {
                                // Apply row background color based on index (striping)
                                let row_color = if i % 2 == 0 { even_row_color } else { odd_row_color };
                                let row_frame = egui::Frame::default()
                                    .fill(row_color)
                                    .inner_margin(Margin::symmetric(8, 4));
                                
                                row_frame.show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        // Index column with dynamic width
                                        ui.add_sized([self.column_widths[0], 20.0], egui::Label::new(file.index.to_string()));
                                        
                                        // File path column (truncated with tooltip)
                                        let path_str = file.path.to_string_lossy();
                                        let truncated_path = crate::ui::UiHandler::truncate_path(&path_str, 50);
                                        
                                        let path_label = ui.add_sized(
                                            [self.column_widths[1], 20.0],
                                            egui::Label::new(
                                                egui::RichText::new(truncated_path)
                                                    .family(egui::FontFamily::Monospace)
                                            )
                                        );
                                        
                                        // Show full path on hover
                                        if path_label.hovered() {
                                            egui::show_tooltip(ui.ctx(), LayerId::background(), egui::Id::new("path_tooltip").with(i), |ui| {
                                                ui.label(path_str);
                                            });
                                        }
                                        
                                        // Token count column (right-aligned with background color)
                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            // Create a colored background based on token count
                                            let token_color = crate::ui::style::token_count_color(
                                                file.tokens,
                                                max_tokens,
                                                self.ui_handler.is_dark_mode()
                                            );
                                            
                                            egui::Frame::default()
                                                .fill(token_color)
                                                .corner_radius(CornerRadius::same(4))
                                                .inner_margin(Margin::symmetric(6, 2))
                                                .show(ui, |ui| {
                                                    ui.add_sized(
                                                        [self.column_widths[2], 20.0],
                                                        egui::Label::new(
                                                            egui::RichText::new(file.tokens.to_string())
                                                                .strong()
                                                                .family(egui::FontFamily::Monospace)
                                                        )
                                                    );
                                                });
                                        });
                                    });
                                });
                                
                                ui.end_row();
                            }
                            
                            // Total row with custom styling
                            let total_files = self.file_list.len();
                            let total_tokens = self.file_list.iter().map(|f| f.tokens).sum::<usize>();
                            
                            let total_frame = egui::Frame::default()
                                .fill(header_color)
                                .inner_margin(Margin::symmetric(8, 4));
                            
                            total_frame.show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    // Empty index cell
                                    ui.add_sized([self.column_widths[0], 20.0], egui::Label::new(""));
                                    
                                    // Total label
                                    ui.add_sized(
                                        [self.column_widths[1], 20.0],
                                        egui::Label::new(
                                            egui::RichText::new(format!("Total: {} files", total_files))
                                                .strong()
                                        )
                                    );
                                    
                                    // Total tokens (right-aligned)
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        ui.add_sized(
                                            [self.column_widths[2], 20.0],
                                            egui::Label::new(
                                                egui::RichText::new(total_tokens.to_string())
                                                    .strong()
                                                    .family(egui::FontFamily::Monospace)
                                            )
                                        );
                                    });
                                });
                            });
                            
                            ui.end_row();
                        });
                });
            }
        });
    }
}

impl GitScrollApp {
    /// Clears the current repository and resets the application state
    fn clear_repository(&mut self) {
        // Clean up the repository if not keeping it
        if !self.keep_repository && self.repository_path.is_some() {
            let _ = self.git_handler.cleanup(self.repository_path.as_ref().unwrap());
        }
        
        // Reset application state
        self.repository_path = None;
        self.directory_structure = None;
        self.file_list.clear();
        self.status_message = String::from("Ready");
        self.is_cloning = false;
        self.is_loading_tokens = false;
        self.ui_handler.set_loading(false);
    }
    
    /// Toggles dark mode
    fn toggle_dark_mode(&mut self) {
        let current_mode = self.ui_handler.is_dark_mode();
        self.ui_handler.set_dark_mode(!current_mode);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    
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
    
    #[test]
    fn test_token_count() {
        // Create a temporary file for testing
        let temp_file = std::env::temp_dir().join("test_token_count.txt");
        fs::write(&temp_file, "hello world this is a test").unwrap();
        
        // Count tokens
        let count = count_tokens(&temp_file);
        assert_eq!(count, 5); // 5 words in the test string
        
        // Clean up
        fs::remove_file(temp_file).unwrap();
    }
    
    #[test]
    fn test_sorting() {
        // Create test file info entries
        let files = vec![
            FileInfo { index: 0, path: PathBuf::from("a.txt"), tokens: 10, selected: false },
            FileInfo { index: 1, path: PathBuf::from("b.txt"), tokens: 5, selected: false },
            FileInfo { index: 2, path: PathBuf::from("c.txt"), tokens: 15, selected: false },
        ];
        
        // Test sorting by tokens ascending
        let mut app = GitScrollApp::new();
        app.file_list = files.clone();
        app.sort_column = SortColumn::Tokens;
        app.sort_direction = SortDirection::Ascending;
        app.sort_file_list();
        assert_eq!(app.file_list[0].tokens, 5);
        assert_eq!(app.file_list[2].tokens, 15);
        
        // Test sorting by tokens descending
        app.sort_direction = SortDirection::Descending;
        app.sort_file_list();
        assert_eq!(app.file_list[0].tokens, 15);
        assert_eq!(app.file_list[2].tokens, 5);
        
        // Test sorting by name
        app.sort_column = SortColumn::Name;
        app.sort_direction = SortDirection::Ascending;
        app.sort_file_list();
        assert_eq!(app.file_list[0].path.file_name().unwrap().to_str().unwrap(), "a.txt");
        assert_eq!(app.file_list[2].path.file_name().unwrap().to_str().unwrap(), "c.txt");
    }
    
    #[test]
    fn test_empty_repository() {
        // Create a temporary directory for testing
        let temp_dir = std::env::temp_dir().join("test_empty_repo");
        if temp_dir.exists() {
            std::fs::remove_dir_all(&temp_dir).unwrap();
        }
        std::fs::create_dir(&temp_dir).unwrap();
        
        // Create a DirectoryEntry for the empty directory
        let root_entry = DirectoryEntry {
            name: "empty".to_string(),
            path: temp_dir.clone(),
            is_directory: true,
            children: vec![],
        };
        
        // Test populating file list with empty repository
        let mut app = GitScrollApp::new();
        app.populate_file_list(&root_entry);
        
        // Verify that the file list is empty
        assert!(app.file_list.is_empty());
        
        // Clean up
        std::fs::remove_dir(temp_dir).unwrap();
    }
    
    #[test]
    fn test_file_with_no_tokens() {
        // Create a temporary file with no content
        let temp_file = std::env::temp_dir().join("empty_file.txt");
        fs::write(&temp_file, "").unwrap();
        
        // Create a DirectoryEntry for the file
        let file_entry = DirectoryEntry {
            name: "empty_file.txt".to_string(),
            path: temp_file.clone(),
            is_directory: false,
            children: vec![],
        };
        
        // Create a parent directory entry
        let root_entry = DirectoryEntry {
            name: "root".to_string(),
            path: temp_file.parent().unwrap().to_path_buf(),
            is_directory: true,
            children: vec![file_entry],
        };
        
        // Test token counting for empty file
        let count = count_tokens(&temp_file);
        assert_eq!(count, 0);
        
        // Clean up
        fs::remove_file(temp_file).unwrap();
    }
}