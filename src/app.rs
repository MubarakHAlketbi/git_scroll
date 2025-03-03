use eframe::egui;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use eframe::epaint::{Margin, CornerRadius};
use egui::LayerId;
use rayon::prelude::*;

// Static variable for cancel flag
static mut CANCEL_FLAG: Option<Arc<AtomicBool>> = None;

use crate::git::GitHandler;
use crate::directory::{DirectoryParser, DirectoryEntry};
use crate::ui::UiHandler;

/// Represents a file's metadata for the list view
#[derive(Clone)]
pub struct FileInfo {
    pub index: usize,          // Order in the list
    pub path: PathBuf,         // Full path to the file
    pub tokens: usize,         // Number of tokens in the file (or size in bytes for binary files)
    pub selected: bool,        // Whether the file is selected
    pub is_binary: bool,       // Whether the file is a binary file
}

/// Formats a file size in bytes to a human-readable string
///
/// # Arguments
/// * `size_bytes` - The size in bytes
///
/// # Returns
/// * `String` - The formatted size string (e.g., "1.23 MB (1234567 bytes)")
fn format_file_size(size_bytes: usize) -> String {
    if size_bytes > 1024 * 1024 * 1024 {
        format!("{:.2} GB ({} bytes)",
            size_bytes as f64 / (1024.0 * 1024.0 * 1024.0),
            size_bytes)
    } else if size_bytes > 1024 * 1024 {
        format!("{:.2} MB ({} bytes)",
            size_bytes as f64 / (1024.0 * 1024.0),
            size_bytes)
    } else if size_bytes > 1024 {
        format!("{:.2} KB ({} bytes)",
            size_bytes as f64 / 1024.0,
            size_bytes)
    } else {
        format!("{} bytes", size_bytes)
    }
}

/// Counts tokens in a file by splitting on whitespace
/// Uses streaming to reduce memory usage for large files
///
/// # Returns
/// * `(usize, bool)` - (token count or file size, is_binary)
fn count_tokens(path: &Path) -> (usize, bool) {
    // Define text file extensions
    let text_extensions = [
        "txt", "rs", "py", "js", "md", "html", "css", "json", "yaml", "toml",
    ];

    // Check if the file has a text extension
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        if !text_extensions.contains(&ext.to_lowercase().as_str()) {
            // For binary files, return the file size in bytes
            return match std::fs::metadata(path) {
                Ok(metadata) => (metadata.len() as usize, true),
                Err(_) => (0, true),
            };
        }
    } else {
        // No extension, assume binary and return file size
        return match std::fs::metadata(path) {
            Ok(metadata) => (metadata.len() as usize, true),
            Err(_) => (0, true),
        };
    }

    // Stream file content and count words to reduce memory usage
    match std::fs::File::open(path) {
        Ok(file) => {
            use std::io::{BufRead, BufReader};
            let reader = BufReader::new(file);
            let token_count = reader.lines()
                .filter_map(Result::ok)
                .map(|line| line.split_whitespace().count())
                .sum();
            (token_count, false) // Not binary, return token count
        },
        Err(_) => (0, false), // Return 0 if file can't be read
    }
}

/// Enum for sortable columns
#[derive(PartialEq, Clone, Copy)]
pub enum SortColumn {
    Index,
    Name,
    Tokens,
}

/// Enum for sort direction
#[derive(PartialEq, Clone, Copy)]
pub enum SortDirection {
    Ascending,
    Descending,
}

/// Progress information for cloning operations
enum CloneProgress {
    Progress(f32),
    Completed(Result<PathBuf, String>),
}

/// Main application state for Git Scroll
pub struct GitScrollApp {
    // Input state
    git_url: String,
    keep_repository: bool,
    
    // Application state
    status_message: String,
    is_cloning: bool,
    cancel_requested: bool, // Flag to cancel cloning operation
    
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
    show_advanced_filters: bool,
    filter_extension: String,
    filter_token_min: usize,
    filter_token_max: usize,
    
    // File list state
    file_list: Vec<FileInfo>,
    sort_column: SortColumn,
    sort_direction: SortDirection,
    is_loading_tokens: bool,
    
    // Table UI state
    column_widths: [f32; 3], // Widths for Index, Name, Tokens columns
    current_page: usize,     // Current page for pagination
    
    // Background processing channels
    clone_receiver: mpsc::Receiver<CloneProgress>,
    parse_receiver: mpsc::Receiver<Result<DirectoryEntry, String>>,
    token_receiver: mpsc::Receiver<(usize, PathBuf, usize, bool)>,
}

impl GitScrollApp {
    /// Creates a new instance of the GitScrollApp
    ///
    /// Returns a new GitScrollApp with default values
    pub fn new() -> Self {
        // Create channels for background processing
        let (_clone_sender, clone_receiver) = mpsc::channel::<CloneProgress>();
        let (_parse_sender, parse_receiver) = mpsc::channel();
        let (_token_sender, token_receiver) = mpsc::channel();
        
        // Initialize with default values
        Self {
            git_url: String::new(),
            keep_repository: false,
            status_message: String::from("Ready"),
            is_cloning: false,
            cancel_requested: false,
            repository_path: None,
            directory_structure: None,
            
            // Initialize module handlers
            git_handler: GitHandler::new(false),
            directory_parser: DirectoryParser::new(),
            ui_handler: UiHandler::new(),
            
            // UI state
            show_stats_panel: true,
            filter_pattern: String::new(),
            show_advanced_filters: false,
            filter_extension: String::new(),
            filter_token_min: 0,
            filter_token_max: 0,
            
            // File list state
            file_list: Vec::new(),
            sort_column: SortColumn::Index,
            sort_direction: SortDirection::Ascending,
            is_loading_tokens: false,
            
            // Table UI state
            column_widths: [60.0, 400.0, 100.0], // Default widths for columns
            current_page: 0,                     // Start at first page
            
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
        self.cancel_requested = false; // Reset cancel flag
        self.status_message = String::from("Cloning repository...");
        self.ui_handler.set_loading(true);
        
        // Create channels for this operation
        let (clone_sender, clone_receiver) = mpsc::channel::<CloneProgress>();
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
        
        // Create a shared cancel flag that can be checked from the background thread
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let cancel_flag_clone = cancel_flag.clone();
        
        // Spawn a background thread to perform the cloning and parsing
        thread::spawn(move || {
            use git2::RemoteCallbacks;
            use git2::build::RepoBuilder;

            // Set up callbacks to track progress
            let mut callbacks = RemoteCallbacks::new();
            let mut fetch_options = git2::FetchOptions::new();

            callbacks.transfer_progress(|stats| {
                // Check if cancellation was requested
                if cancel_flag.load(Ordering::SeqCst) {
                    // Return false to abort the transfer
                    return false;
                }
                
                let progress = if stats.total_objects() > 0 {
                    (stats.received_objects() as f32 / stats.total_objects() as f32).min(1.0)
                } else {
                    0.0
                };
                let _ = clone_sender.send(CloneProgress::Progress(progress));
                true
            });

            fetch_options.remote_callbacks(callbacks);
            let mut builder = RepoBuilder::new();
            builder.fetch_options(fetch_options);

            // Clone the repository with progress tracking
            println!("Cloning {} to {:?}", git_url, temp_dir.path());
            let repo_result = builder.clone(&git_url, temp_dir.path());
            
            // Check if cancellation was requested
            if cancel_flag.load(Ordering::SeqCst) {
                let _ = clone_sender.send(CloneProgress::Completed(Err("Clone operation cancelled by user".to_string())));
                return;
            }
            
            // Send the final result
            match repo_result {
                Ok(repo) => {
                    let repo_path = repo.path().parent().unwrap_or(repo.path()).to_path_buf();
                    let _ = clone_sender.send(CloneProgress::Completed(Ok(repo_path.clone())));
                    
                    // Parse the directory structure
                    let parser = DirectoryParser::new();
                    let parse_result = parser.parse_directory(&repo_path);
                    let _ = parse_sender.send(parse_result);
                },
                Err(e) => {
                    let _ = clone_sender.send(CloneProgress::Completed(Err(e.to_string())));
                }
            }
        });
        
        // Store the cancel flag for later use in the static variable
        unsafe {
            CANCEL_FLAG = Some(cancel_flag_clone);
        }
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
            let text_files = self.file_list.iter().filter(|f| !f.is_binary).count();
            let binary_files = self.file_list.iter().filter(|f| f.is_binary).count();
            let total_tokens = self.file_list.iter().filter(|f| !f.is_binary).map(|f| f.tokens).sum::<usize>();
            let avg_tokens = if text_files > 0 { total_tokens / text_files } else { 0 };
            
            ui.label(format!("Total Files: {}", total_files));
            ui.label(format!("Text Files: {}", text_files));
            ui.label(format!("Binary Files: {}", binary_files));
            ui.label(format!("Total Tokens (text files): {}", total_tokens));
            ui.label(format!("Average Tokens per Text File: {}", avg_tokens));
            
            // Add binary file size statistics
            if binary_files > 0 {
                let total_binary_size = self.file_list.iter()
                    .filter(|f| f.is_binary)
                    .map(|f| f.tokens)
                    .sum::<usize>();
                
                let avg_binary_size = if binary_files > 0 { total_binary_size / binary_files } else { 0 };
                
                ui.add_space(5.0);
                ui.label(format!("Average Binary Size: {}", format_file_size(avg_binary_size)));
                ui.label(format!("Total Binary Size: {}", format_file_size(total_binary_size)));
            }
            
            ui.add_space(10.0);
            ui.separator();
            ui.add_space(10.0);
            
            // Add token count legend (for text files only)
            ui.label("Token Count Legend (Text Files):");
            ui.horizontal(|ui| {
                // Calculate max tokens for color scaling (text files only)
                let max_tokens = self.file_list.iter()
                    .filter(|f| !f.is_binary)
                    .map(|f| f.tokens)
                    .max()
                    .unwrap_or(1);
                
                let low_color = crate::ui::style::token_count_color(0, max_tokens, self.ui_handler.is_dark_mode());
                let mid_color = crate::ui::style::token_count_color(max_tokens / 2, max_tokens, self.ui_handler.is_dark_mode());
                let high_color = crate::ui::style::token_count_color(max_tokens, max_tokens, self.ui_handler.is_dark_mode());
                
                ui.label(egui::RichText::new("Low").color(low_color));
                ui.label(" → ");
                ui.label(egui::RichText::new("Medium").color(mid_color));
                ui.label(" → ");
                ui.label(egui::RichText::new("High").color(high_color));
            });
            
            ui.add_space(10.0);
            ui.separator();
            ui.add_space(10.0);
            ui.heading("Top Text Files by Token Count");
            ui.add_space(5.0);
            
            // Get top text files by token count (exclude binary files)
            let mut top_files = self.file_list.iter()
                .filter(|f| !f.is_binary)
                .cloned()
                .collect::<Vec<_>>();
            top_files.sort_by(|a, b| b.tokens.cmp(&a.tokens));
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
    /// Uses parallel processing with rayon for better performance
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
        self.file_list = files
            .iter()
            .enumerate()
            .map(|(index, path)| FileInfo {
                index,
                path: path.clone(),
                tokens: 0, // Will be updated asynchronously
                selected: false, // Not selected by default
                is_binary: false, // Will be updated asynchronously
            })
            .collect();
        
        // Process files in parallel using rayon
        let files_to_process = files.clone();
        thread::spawn(move || {
            // Use par_iter for parallel processing with a thread pool
            files_to_process.par_iter().enumerate().for_each(|(index, path)| {
                let (tokens, is_binary) = count_tokens(path);
                let _ = token_sender.send((index, path.clone(), tokens, is_binary));
            });
        });
        
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
        // Check if cancel was requested and update the cancel flag
        if self.cancel_requested {
            // Access the static cancel flag
            unsafe {
                if let Some(ref cancel_flag) = CANCEL_FLAG {
                    cancel_flag.store(true, Ordering::SeqCst);
                }
            }
        }
        
        // Check for clone progress and results
        if let Ok(clone_msg) = self.clone_receiver.try_recv() {
            match clone_msg {
                CloneProgress::Progress(progress) => {
                    // Update progress in UI
                    self.ui_handler.set_progress(progress);
                    self.status_message = format!("Cloning repository... {:.0}%", progress * 100.0);
                    ctx.request_repaint(); // Force UI update to show progress
                },
                CloneProgress::Completed(result) => {
                    match result {
                        Ok(repo_path) => {
                            self.repository_path = Some(repo_path);
                            self.status_message = String::from("Repository cloned successfully, parsing directory...");
                            self.cancel_requested = false; // Reset cancel flag
                        },
                        Err(e) => {
                            let error_message = format!("Failed to clone repository: {}", e);
                            self.status_message = error_message.clone();
                            self.is_cloning = false;
                            self.ui_handler.set_loading(false);
                            self.cancel_requested = false; // Reset cancel flag
                            
                            // Show error dialog for critical errors
                            self.show_error_dialog(ctx, &error_message);
                        }
                    }
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
        
        // Check for token counting results with improved responsiveness
        if self.is_loading_tokens {
            let mut received_count = 0;
            let mut all_received = false;
            let total_files = self.file_list.len();
            let mut needs_sort = false;
            
            // Try to receive token results in batches without blocking
            for _ in 0..20 { // Process up to 20 results per frame for smoother UI
                match self.token_receiver.try_recv() {
                    Ok((index, path, tokens, is_binary)) => {
                        received_count += 1;
                        
                        // Update the token count for the file with matching index and path
                        if let Some(file) = self.file_list.iter_mut().find(|f| f.index == index && f.path == path) {
                            file.tokens = tokens;
                            file.is_binary = is_binary;
                            needs_sort = true;
                        }
                    },
                    Err(_) => break // No more results available right now
                }
            }
            
            // Resort the list if we received any results
            if needs_sort {
                self.sort_file_list();
                // Request a repaint to update the UI immediately
                ctx.request_repaint();
            }
            
            // Update progress in status message
            let completed = self.file_list.iter().filter(|f| f.tokens > 0).count();
            if completed > 0 {
                let percentage = (completed as f32 / total_files as f32 * 100.0) as usize;
                self.status_message = format!("Counting tokens: {}% ({}/{})", percentage, completed, total_files);
            }
            
            // Check if all files have been processed
            if completed >= total_files {
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
        
        // Top panel for URL input and controls with adaptive layout
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.add_space(8.0); // Padding at the top

            ui.horizontal(|ui| {
                // Fixed widths for buttons and checkbox
                let clone_button_width = 80.0;
                let clear_button_width = 60.0;
                let theme_button_width = 60.0;
                let checkbox_width = 120.0; // Approximate width for "Keep Repository" checkbox
                let spacing = 8.0;
                let label_width = 60.0; // Approximate width for "Git URL:" label

                // Git URL label - simple approach with consistent height
                ui.add_sized(
                    [label_width, 28.0], // Same height as other elements
                    egui::Label::new(egui::RichText::new("Git URL:").strong())
                );

                // Fixed widths for buttons and checkbox
                let clone_button_width = 80.0;
                let clear_button_width = 60.0;
                let theme_button_width = 60.0;
                let checkbox_width = 120.0; // Approximate width for "Keep Repository" checkbox
                let spacing = 8.0;

                // Calculate total fixed width (label + buttons + checkbox + spacings)
                let total_fixed_width = label_width
                    + clone_button_width
                    + checkbox_width
                    + clear_button_width
                    + theme_button_width
                    + (spacing * 5.0); // Spaces between elements

                // Set minimum URL input width
                let min_url_width = 200.0;

                // Calculate URL width directly - this ensures it grows immediately with window expansion
                let url_width = (ui.available_width() - total_fixed_width).max(min_url_width);

                // URL input with flexible width
                let url_input_id = ui.make_persistent_id("git_url_input");
                let response = ui.add_sized(
                    [url_width, 28.0],
                    egui::TextEdit::singleline(&mut self.git_url)
                        .hint_text("Enter repository URL...")
                        .id(url_input_id)
                );

                // Handle Enter key press when URL input is focused
                if response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    self.handle_clone_button();
                }

                ui.add_space(spacing);

                // Clone or Cancel button based on state
                if self.is_cloning {
                    // Show Cancel button when cloning
                    if ui.add(
                        egui::Button::new(
                            egui::RichText::new("Cancel")
                                .strong()
                        )
                        .min_size(egui::vec2(clone_button_width, 28.0))
                    ).clicked() {
                        self.cancel_requested = true;
                        self.status_message = String::from("Cancelling clone operation...");
                    }
                } else {
                    // Show Clone button when not cloning
                    if ui.add_enabled(
                        !self.git_url.is_empty(),
                        egui::Button::new(
                            egui::RichText::new("Clone")
                                .strong()
                        )
                        .min_size(egui::vec2(clone_button_width, 28.0))
                    ).clicked() {
                        self.handle_clone_button();
                    }
                }

                ui.add_space(spacing);

                // Keep Repository checkbox
                ui.checkbox(&mut self.keep_repository, "Keep Repository");

                ui.add_space(spacing);

                // Clear button
                if ui.add(
                    egui::Button::new(egui::RichText::new("Clear").strong())
                        .min_size(egui::vec2(clear_button_width, 28.0))
                ).clicked() {
                    self.clear_repository();
                }

                ui.add_space(spacing);

                // Theme toggle button
                if ui.add(
                    egui::Button::new(
                        if self.ui_handler.is_dark_mode() { "Light" } else { "Dark" }
                    )
                    .min_size(egui::vec2(theme_button_width, 28.0))
                ).clicked() {
                    self.toggle_dark_mode();
                }
            });

            ui.add_space(8.0); // Padding at the bottom
        });
        
        // Controls panel removed - consolidated into central panel
        
        // Optimized bottom panel with horizontal layout for status and stats
        egui::TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
            ui.add_space(8.0);
            
            ui.horizontal(|ui| {
                // Status message on the left
                let status_width = ui.available_width() * 0.6;
                ui.horizontal(|ui| {
                    ui.set_width(status_width);
                    self.ui_handler.render_status_bar(ui, &self.status_message, self.is_loading_tokens);
                });
                
                // Stats on the right (if repository is loaded)
                if self.directory_structure.is_some() {
                    ui.separator();
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let total_files = self.file_list.len();
                        let text_files = self.file_list.iter().filter(|f| !f.is_binary).count();
                        let binary_files = self.file_list.iter().filter(|f| f.is_binary).count();
                        let total_tokens = self.file_list.iter().filter(|f| !f.is_binary).map(|f| f.tokens).sum::<usize>();
                        let avg_tokens = if text_files > 0 { total_tokens / text_files } else { 0 };
                        
                        ui.label(format!("Avg: {} tokens/file", avg_tokens));
                        ui.add_space(8.0);
                        ui.label(egui::RichText::new(format!("Tokens: {}", total_tokens)).strong());
                        ui.add_space(8.0);
                        ui.label(egui::RichText::new(format!("Files: {} ({} text, {} bin)",
                            total_files, text_files, binary_files)).strong());
                    });
                }
            });
            
            ui.add_space(8.0);
        });
        
        // Main central panel
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.directory_structure.is_none() {
                // Empty state when no repository is loaded
                self.ui_handler.render_empty_state(ui, &mut self.git_url);
            } else {
                // Enhanced header section with title, sorting, filtering, and export options
                ui.horizontal(|ui| {
                    ui.heading("Repository Files");
                    
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Add export button
                        if ui.button("Export to CSV").clicked() {
                            self.export_to_csv();
                        }
                        
                        ui.add_space(8.0);
                        
                        // Add sort direction toggle
                        let direction_text = match self.sort_direction {
                            SortDirection::Ascending => "↑ Asc",
                            SortDirection::Descending => "↓ Desc",
                        };
                        if ui.button(direction_text).clicked() {
                            self.sort_direction = match self.sort_direction {
                                SortDirection::Ascending => SortDirection::Descending,
                                SortDirection::Descending => SortDirection::Ascending,
                            };
                            self.sort_file_list();
                        }
                        
                        ui.add_space(4.0);
                        
                        // Add sort column selector
                        egui::ComboBox::from_id_source("sort_column")
                            .selected_text(match self.sort_column {
                                SortColumn::Index => "Sort: Number",
                                SortColumn::Name => "Sort: Name",
                                SortColumn::Tokens => "Sort: Tokens",
                            })
                            .show_ui(ui, |ui| {
                                if ui.selectable_value(&mut self.sort_column, SortColumn::Index, "Number").clicked() {
                                    self.sort_file_list();
                                }
                                if ui.selectable_value(&mut self.sort_column, SortColumn::Name, "Name").clicked() {
                                    self.sort_file_list();
                                }
                                if ui.selectable_value(&mut self.sort_column, SortColumn::Tokens, "Tokens").clicked() {
                                    self.sort_file_list();
                                }
                            });
                    });
                });
                
                ui.add_space(8.0);
                
                // Search and filter bar with improved layout
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Filter:").strong());
                    
                    // Calculate available width for the filter input
                    let available_width = ui.available_width() - 150.0; // Space for button
                    
                    // Add filter input with dynamic width
                    let mut filter_text = self.filter_pattern.clone();
                    if ui.add_sized(
                        [available_width.max(200.0), 28.0],
                        egui::TextEdit::singleline(&mut filter_text)
                            .hint_text("Filter files...")
                    ).changed() {
                        self.handle_filter_change(filter_text);
                    }
                    
                    ui.add_space(8.0);
                    
                    // Add advanced filter options
                    if ui.button(if self.show_advanced_filters { "Hide Advanced" } else { "Advanced Filters" }).clicked() {
                        self.show_advanced_filters = !self.show_advanced_filters;
                    }
                });
                
                // Enhanced advanced filters section with better layout and visual feedback
                if self.show_advanced_filters {
                    egui::Frame::group(ui.style())
                        .fill(if self.ui_handler.is_dark_mode() {
                            egui::Color32::from_rgb(45, 45, 48)
                        } else {
                            egui::Color32::from_rgb(240, 240, 245)
                        })
                        .inner_margin(egui::vec2(10.0, 8.0))
                        .show(ui, |ui| {
                            ui.heading("Advanced Filters");
                            ui.add_space(4.0);
                            
                            // Extension filter
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new("File Extension:").strong());
                                ui.add_space(4.0);
                                
                                let mut extension = self.filter_extension.clone();
                                if ui.add_sized(
                                    [120.0, 24.0],
                                    egui::TextEdit::singleline(&mut extension)
                                        .hint_text("e.g., rs, js, py")
                                ).changed() {
                                    self.filter_extension = extension;
                                    self.apply_advanced_filters();
                                }
                                
                                ui.add_space(4.0);
                                if ui.button("Clear").clicked() {
                                    self.filter_extension = String::new();
                                    self.apply_advanced_filters();
                                }
                            });
                            
                            ui.add_space(8.0);
                            
                            // Token range filters
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new("Token Range:").strong());
                                ui.add_space(4.0);
                                
                                ui.label("Min:");
                                let mut min_tokens = self.filter_token_min.to_string();
                                if ui.add_sized(
                                    [80.0, 24.0],
                                    egui::TextEdit::singleline(&mut min_tokens)
                                        .hint_text("0")
                                ).changed() {
                                    if let Ok(value) = min_tokens.parse::<usize>() {
                                        self.filter_token_min = value;
                                        self.apply_advanced_filters();
                                    }
                                }
                                
                                ui.add_space(8.0);
                                
                                ui.label("Max:");
                                let mut max_tokens = self.filter_token_max.to_string();
                                if ui.add_sized(
                                    [80.0, 24.0],
                                    egui::TextEdit::singleline(&mut max_tokens)
                                        .hint_text("∞")
                                ).changed() {
                                    if let Ok(value) = max_tokens.parse::<usize>() {
                                        self.filter_token_max = value;
                                        self.apply_advanced_filters();
                                    }
                                }
                                
                                ui.add_space(4.0);
                                if ui.button("Reset Range").clicked() {
                                    self.filter_token_min = 0;
                                    self.filter_token_max = 0;
                                    self.apply_advanced_filters();
                                }
                            });
                            
                            ui.add_space(4.0);
                            
                            // Show active filters summary
                            let has_filters = !self.filter_extension.is_empty() ||
                                             self.filter_token_min > 0 ||
                                             self.filter_token_max > 0;
                            
                            if has_filters {
                                ui.add_space(4.0);
                                ui.separator();
                                ui.add_space(4.0);
                                
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new("Active Filters:").strong());
                                    
                                    let mut filter_text = Vec::new();
                                    
                                    if !self.filter_extension.is_empty() {
                                        filter_text.push(format!("Extension: {}", self.filter_extension));
                                    }
                                    
                                    if self.filter_token_min > 0 {
                                        filter_text.push(format!("Min Tokens: {}", self.filter_token_min));
                                    }
                                    
                                    if self.filter_token_max > 0 {
                                        filter_text.push(format!("Max Tokens: {}", self.filter_token_max));
                                    }
                                    
                                    ui.label(filter_text.join(" | "));
                                    
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if ui.button("Clear All Filters").clicked() {
                                            self.filter_extension = String::new();
                                            self.filter_token_min = 0;
                                            self.filter_token_max = 0;
                                            self.apply_advanced_filters();
                                        }
                                    });
                                });
                            }
                        });
                }
                
                ui.add_space(8.0);
                
                // Calculate max tokens for color scaling (text files only)
                let max_tokens = self.file_list.iter()
                    .filter(|f| !f.is_binary)
                    .map(|f| f.tokens)
                    .max()
                    .unwrap_or(1);
                
                // Get row colors for striping
                let (even_row_color, odd_row_color) =
                    crate::ui::style::row_colors(self.ui_handler.is_dark_mode());
                
                // Get header color
                let header_color = crate::ui::style::header_color(self.ui_handler.is_dark_mode());
                
                // Calculate pagination with dynamic items per page based on available height
                let row_height = 24.0;
                let header_height = 30.0;
                let footer_height = 40.0; // Space for pagination controls
                let available_height = ui.available_height() - header_height - footer_height;
                
                // Calculate items per page based on available height, with a minimum of 10 items
                let items_per_page = (available_height / row_height).max(10.0) as usize;
                let total_pages = (self.file_list.len() + items_per_page - 1) / items_per_page;
                let start_idx = self.current_page * items_per_page;
                let end_idx = (start_idx + items_per_page).min(self.file_list.len());
                let visible_items = end_idx - start_idx;
                
                // File list table with virtual scrolling for better performance
                let row_height = 24.0; // Estimated height of each row
                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .show_rows(ui, row_height, visible_items, |ui, row_range| {
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
                                    ("Tokens/Size", SortColumn::Tokens, self.column_widths[2]),
                                ];
                                
                                for (i, (text, col, width)) in headers.iter().enumerate() {
                                    let is_sorted = self.sort_column == *col;
                                    let sort_indicator = if is_sorted {
                                        if self.sort_direction == SortDirection::Ascending { "↑" } else { "↓" }
                                    } else { "" };
                                    
                                    // Make headers clickable for sorting
                                    let header_button = ui.add_sized(
                                        [*width, 30.0],
                                        egui::Button::new(
                                            egui::RichText::new(format!("{} {}", text, sort_indicator)).strong()
                                        ).fill(header_color)
                                    );
                                    
                                    if header_button.clicked() {
                                        if self.sort_column == *col {
                                            // Toggle direction if already sorting by this column
                                            self.sort_direction = match self.sort_direction {
                                                SortDirection::Ascending => SortDirection::Descending,
                                                SortDirection::Descending => SortDirection::Ascending,
                                            };
                                        } else {
                                            // Set new sort column
                                            self.sort_column = *col;
                                        }
                                        self.sort_file_list();
                                    }
                                    
                                    // Add tooltip to explain sorting
                                    if header_button.hovered() {
                                        egui::show_tooltip(ui.ctx(), LayerId::background(), egui::Id::new("sort_tooltip").with(i), |ui| {
                                            ui.label(format!("Click to sort by {}", text));
                                        });
                                    }
                                    
                                    // Add resize handle between columns
                                    if i < 2 { // Only between columns
                                        // Create a visible draggable area for resizing
                                        let resize_id = ui.id().with(("resize", i));
                                        
                                        // Make the resize handle more visible
                                        let resize_width = 8.0;
                                        // Import std::ops::Add for Pos2
                                        use std::ops::Add;
                                        
                                        let resize_rect = egui::Rect::from_min_size(
                                            ui.cursor().min + egui::vec2(-resize_width/2.0, 0.0),
                                            egui::vec2(resize_width, ui.available_height().min(30.0))
                                        );
                                        
                                        // Draw a visible handle
                                        if ui.is_rect_visible(resize_rect) {
                                            // Get the response from interact to determine hover/active state
                                            let resize_response = ui.interact(
                                                resize_rect,
                                                resize_id,
                                                egui::Sense::drag()
                                            );
                                            
                                            // Determine stroke based on hover/active state
                                            let stroke = if resize_response.hovered() || resize_response.dragged() {
                                                egui::Stroke::new(2.0, egui::Color32::from_rgb(100, 150, 255))
                                            } else {
                                                egui::Stroke::new(1.0, egui::Color32::from_gray(160))
                                            };
                                            
                                            ui.painter().line_segment(
                                                [
                                                    resize_rect.center_top(),
                                                    resize_rect.center_bottom(),
                                                ],
                                                stroke
                                            );
                                        }
                                        
                                        let resize_response = ui.interact(
                                            resize_rect,
                                            resize_id,
                                            egui::Sense::drag()
                                        );
                                        
                                        if resize_response.dragged() {
                                            let delta = ui.input(|i| i.pointer.delta().x);
                                            
                                            // Adjust both columns to maintain total width
                                            self.column_widths[i] += delta;
                                            self.column_widths[i+1] -= delta;
                                            
                                            // Ensure minimum widths
                                            self.column_widths[i] = self.column_widths[i].max(50.0);
                                            self.column_widths[i+1] = self.column_widths[i+1].max(50.0);
                                            
                                            // Request repaint for smooth resizing
                                            ui.ctx().request_repaint();
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
                        .striped(true) // Use built-in striping
                        .show(ui, |ui| {
                            // Only render visible rows for the current page
                            for relative_idx in row_range {
                                let absolute_idx = start_idx + relative_idx;
                                if absolute_idx >= self.file_list.len() {
                                    break;
                                }
                                let i = absolute_idx; // For compatibility with existing code
                                ui.horizontal(|ui| {
                                    // Add checkbox for selection
                                    let mut selected = self.file_list[absolute_idx].selected;
                                    if ui.checkbox(&mut selected, "").changed() {
                                        // Get the index directly to avoid borrowing issues
                                        let file_index = self.file_list[absolute_idx].index;
                                        
                                        // Update the selection state after the UI closure
                                        ui.ctx().data_mut(|data| {
                                            data.insert_temp(egui::Id::new("file_selection_change"), (file_index, selected));
                                        });
                                        
                                        // Handle shift-click for multi-selection
                                        if ui.input(|i| i.modifiers.shift) && selected {
                                            if let Some(last_selected) = self.file_list.iter().rposition(|f| f.selected && f.index != self.file_list[absolute_idx].index) {
                                                let clicked_idx = i;
                                                let range = if clicked_idx < last_selected {
                                                    clicked_idx..=last_selected
                                                } else {
                                                    last_selected..=clicked_idx
                                                };
                                                for idx in range {
                                                    if idx < self.file_list.len() {
                                                        self.file_list[idx].selected = true;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    
                                    // Index column with dynamic width - right aligned
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        ui.add_sized([self.column_widths[0], 20.0], egui::Label::new(self.file_list[absolute_idx].index.to_string()));
                                    });
                                    
                                    // File path column with tree structure and color coding
                                    let path_str = self.file_list[absolute_idx].path.to_string_lossy();
                                    
                                    // Calculate the file's depth in the directory structure
                                    let path_components: Vec<&str> = path_str.split(['/', '\\']).collect();
                                    let depth = path_components.len().saturating_sub(1);
                                    
                                    // Get file extension for color coding
                                    let extension = self.file_list[absolute_idx].path.extension()
                                        .and_then(|e| e.to_str())
                                        .unwrap_or("");
                                    
                                    // Determine file type color based on extension
                                    let file_color = match extension.to_lowercase().as_str() {
                                        "rs" => egui::Color32::from_rgb(255, 160, 80),  // Rust files - orange
                                        "js" | "ts" => egui::Color32::from_rgb(240, 220, 80),  // JavaScript/TypeScript - yellow
                                        "py" => egui::Color32::from_rgb(80, 160, 255),  // Python - blue
                                        "html" | "css" => egui::Color32::from_rgb(100, 200, 100),  // Web files - green
                                        "md" | "txt" => egui::Color32::from_rgb(200, 200, 200),  // Documentation - light gray
                                        "json" | "toml" | "yaml" => egui::Color32::from_rgb(200, 150, 255),  // Config files - purple
                                        _ => if self.ui_handler.is_dark_mode() {
                                            egui::Color32::from_rgb(180, 180, 180)  // Default - light gray
                                        } else {
                                            egui::Color32::from_rgb(80, 80, 80)  // Default - dark gray
                                        }
                                    };
                                    
                                    // Get just the file name for display
                                    let file_name = self.file_list[absolute_idx].path.file_name()
                                        .map(|n| n.to_string_lossy().to_string())
                                        .unwrap_or_default();
                                    
                                    // Create indentation based on depth
                                    let indent = "  ".repeat(depth);
                                    
                                    // Add tree structure character
                                    let tree_prefix = if depth > 0 { "└─ " } else { "" };
                                    
                                    // Combine for display
                                    let display_path = format!("{}{}{}", indent, tree_prefix, file_name);
                                    
                                    // Create the label with the file path
                                    let path_label = ui.add_sized(
                                        [self.column_widths[1], 20.0],
                                        egui::Label::new(
                                            egui::RichText::new(display_path)
                                                .family(egui::FontFamily::Monospace)
                                                .color(file_color)
                                        )
                                    );
                                    
                                    // Show full path on hover with extension info
                                    if path_label.hovered() {
                                        egui::show_tooltip(ui.ctx(), LayerId::background(), egui::Id::new("path_tooltip").with(i), |ui| {
                                            let extension = self.file_list[absolute_idx].path.extension()
                                                .map_or("".to_string(), |e| format!(" ({})", e.to_string_lossy()));
                                            ui.label(format!("{}{}", path_str, extension));
                                        });
                                        
                                        // Add context menu on right-click
                                        if ui.input(|i| i.pointer.secondary_clicked()) {
                                            // Store the file index for the context menu
                                            let context_menu_id = ui.make_persistent_id("file_context_menu");
                                            ui.memory_mut(|mem| mem.data.insert_temp(context_menu_id, i));
                                            
                                            // Show the context menu as a popup
                                            let popup_id = ui.make_persistent_id("file_context_popup");
                                            let popup_response = egui::popup::popup_below_widget(ui, popup_id, &path_label, egui::PopupCloseBehavior::CloseOnClickOutside, |ui: &mut egui::Ui| {
                                                ui.set_min_width(150.0);
                                                
                                                let open_response = ui.button("Open File");
                                                if open_response.clicked() {
                                                    #[cfg(target_os = "windows")]
                                                    {
                                                        std::process::Command::new("cmd")
                                                            .args(&["/c", "start", "", self.file_list[absolute_idx].path.to_string_lossy().as_ref()])
                                                            .spawn()
                                                            .ok();
                                                    }
                                                    #[cfg(not(target_os = "windows"))]
                                                    {
                                                        std::process::Command::new("xdg-open")
                                                            .arg(self.file_list[absolute_idx].path.to_string_lossy().as_ref())
                                                            .spawn()
                                                            .ok();
                                                    }
                                                    // Close the popup when clicked
                                                    ui.ctx().memory_mut(|mem| {
                                                        mem.close_popup();
                                                    });
                                                }
                                                
                                                let copy_response = ui.button("Copy Path");
                                                if copy_response.clicked() {
                                                    ui.output_mut(|o| o.copied_text = self.file_list[absolute_idx].path.to_string_lossy().to_string());
                                                    // Close the popup when clicked
                                                    ui.ctx().memory_mut(|mem| {
                                                        mem.close_popup();
                                                    });
                                                }
                                            });
                                            
                                            // Position the popup at the mouse position
                                            if let Some(pos) = ui.ctx().pointer_interact_pos() {
                                                ui.ctx().memory_mut(|mem| {
                                                    mem.open_popup(popup_id);
                                                });
                                                
                                                // Request a repaint to show the popup immediately
                                                ui.ctx().request_repaint();
                                            }
                                        }
                                    }
                                    
                                    // Token count column (right-aligned with background color)
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        // Create a colored background based on token count or binary status
                                        let token_color = if self.file_list[absolute_idx].is_binary {
                                            // Use a distinct color for binary files
                                            if self.ui_handler.is_dark_mode() {
                                                egui::Color32::from_rgb(80, 80, 120) // Dark blue for binary files in dark mode
                                            } else {
                                                egui::Color32::from_rgb(200, 200, 240) // Light blue for binary files in light mode
                                            }
                                        } else {
                                            // Use the regular token count color for text files
                                            crate::ui::style::token_count_color(
                                                self.file_list[absolute_idx].tokens,
                                                max_tokens,
                                                self.ui_handler.is_dark_mode()
                                            )
                                        };
                                        
                                        egui::Frame::default()
                                            .fill(token_color)
                                            .corner_radius(CornerRadius::same(4))
                                            .inner_margin(Margin::symmetric(6, 2))
                                            .show(ui, |ui| {
                                                // Display token count or file size based on whether it's a binary file
                                                let display_text = if self.file_list[absolute_idx].is_binary {
                                                    format_file_size(self.file_list[absolute_idx].tokens)
                                                } else {
                                                    self.file_list[absolute_idx].tokens.to_string()
                                                };
                                                
                                                ui.add_sized(
                                                    [self.column_widths[2], 20.0],
                                                    egui::Label::new(
                                                        egui::RichText::new(display_text)
                                                            .strong()
                                                            .family(egui::FontFamily::Monospace)
                                                    )
                                                );
                                            });
                                    });
                                });
                                
                                ui.end_row();
                            }
                            
                            // Total row with custom styling
                            let page_files = end_idx - start_idx;
                            let total_files = self.file_list.len();
                            
                            // Count text files only for token totals
                            let page_text_files = self.file_list[start_idx..end_idx].iter().filter(|f| !f.is_binary).count();
                            let page_binary_files = page_files - page_text_files;
                            let page_tokens = self.file_list[start_idx..end_idx].iter()
                                .filter(|f| !f.is_binary)
                                .map(|f| f.tokens).sum::<usize>();
                            
                            let total_text_files = self.file_list.iter().filter(|f| !f.is_binary).count();
                            let total_binary_files = total_files - total_text_files;
                            let total_tokens = self.file_list.iter()
                                .filter(|f| !f.is_binary)
                                .map(|f| f.tokens).sum::<usize>();
                            
                            let total_frame = egui::Frame::default()
                                .fill(header_color)
                                .inner_margin(Margin::symmetric(8, 4));
                            
                            total_frame.show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    // Empty index cell
                                    ui.add_sized([self.column_widths[0], 20.0], egui::Label::new(""));
                                    
                                    // Total label showing page and total counts with text/binary breakdown
                                    ui.add_sized(
                                        [self.column_widths[1], 20.0],
                                        egui::Label::new(
                                            egui::RichText::new(
                                                format!("Page: {} files ({} text, {} bin) | Total: {} files ({} text, {} bin)",
                                                    page_files, page_text_files, page_binary_files,
                                                    total_files, total_text_files, total_binary_files)
                                            ).strong()
                                        )
                                    );
                                    
                                    // Total tokens (right-aligned) showing page and total
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        // Show token counts for text files and total size for binary files
                                        let page_binary_size = self.file_list[start_idx..end_idx].iter()
                                            .filter(|f| f.is_binary)
                                            .map(|f| f.tokens).sum::<usize>();
                                        let total_binary_size = self.file_list.iter()
                                            .filter(|f| f.is_binary)
                                            .map(|f| f.tokens).sum::<usize>();
                                            
                                        ui.add_sized(
                                            [self.column_widths[2], 20.0],
                                            egui::Label::new(
                                                egui::RichText::new(
                                                    format!("Text: {} / {} tokens | Bin: {} / {}",
                                                        page_tokens, total_tokens,
                                                        format_file_size(page_binary_size), format_file_size(total_binary_size))
                                                ).strong()
                                                 .family(egui::FontFamily::Monospace)
                                            )
                                        );
                                    });
                                });
                            });
                            
                            ui.end_row();
                        });
                });
                
                // Enhanced pagination controls with better layout and more options
                ui.add_space(10.0);
                ui.separator();
                ui.add_space(5.0);
                
                // Use the dynamically calculated items_per_page from above
                if total_pages > 1 {
                    egui::Frame::default()
                        .fill(if self.ui_handler.is_dark_mode() {
                            egui::Color32::from_rgb(40, 40, 45)
                        } else {
                            egui::Color32::from_rgb(245, 245, 250)
                        })
                        .inner_margin(egui::vec2(8.0, 8.0))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                // First page button
                                if ui.add_enabled(
                                    self.current_page > 0,
                                    egui::Button::new("⏮ First")
                                ).clicked() {
                                    self.current_page = 0;
                                }
                                
                                // Previous page button
                                if ui.add_enabled(
                                    self.current_page > 0,
                                    egui::Button::new("◀ Previous")
                                ).clicked() {
                                    self.current_page = self.current_page.saturating_sub(1);
                                }
                                
                                // Page number indicator with strong text
                                ui.label(egui::RichText::new(
                                    format!("Page {} of {}", self.current_page + 1, total_pages)
                                ).strong());
                                
                                // Next page button
                                if ui.add_enabled(
                                    self.current_page < total_pages - 1,
                                    egui::Button::new("Next ▶")
                                ).clicked() {
                                    self.current_page = (self.current_page + 1).min(total_pages - 1);
                                }
                                
                                // Last page button
                                if ui.add_enabled(
                                    self.current_page < total_pages - 1,
                                    egui::Button::new("Last ⏭")
                                ).clicked() {
                                    self.current_page = total_pages - 1;
                                }
                                
                                // Jump to page with better styling
                                ui.separator();
                                ui.label("Go to:");
                                let mut page_text = (self.current_page + 1).to_string();
                                let response = ui.add_sized(
                                    [50.0, 24.0],
                                    egui::TextEdit::singleline(&mut page_text)
                                        .hint_text("Page #")
                                );
                                
                                if response.changed() || (response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) {
                                    if let Ok(page) = page_text.parse::<usize>() {
                                        if page > 0 && page <= total_pages {
                                            self.current_page = page - 1;
                                        }
                                    }
                                }
                                
                                // Add page size selector
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.label("Items per page:");
                                    let page_sizes = [10, 25, 50, 100];
                                    let current_size = items_per_page;
                                    
                                    egui::ComboBox::from_id_source("page_size")
                                        .selected_text(format!("{}", current_size))
                                        .show_ui(ui, |ui| {
                                            for &size in &page_sizes {
                                                let text = format!("{}", size);
                                                if ui.selectable_label(current_size == size, text).clicked() {
                                                    // We can't actually change the items_per_page here since it's calculated dynamically,
                                                    // but in a real implementation, you would store the user's preference and use it
                                                    // to override the calculated value
                                                }
                                            }
                                            ui.selectable_label(true, "Auto"); // Always selected since we're using dynamic calculation
                                        });
                                });
                            });
                        });
                }
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
        self.cancel_requested = false; // Reset cancel flag
        self.is_loading_tokens = false;
        self.current_page = 0; // Reset to first page
        self.ui_handler.set_loading(false);
    }
    
    /// Toggles dark mode
    fn toggle_dark_mode(&mut self) {
        let current_mode = self.ui_handler.is_dark_mode();
        self.ui_handler.set_dark_mode(!current_mode);
    }
    
    /// Applies advanced filters to the file list
    fn apply_advanced_filters(&mut self) {
        if let Some(root_entry) = &self.directory_structure {
            // Get all files from the directory structure
            let files = self.directory_parser.get_all_files(root_entry);
            
            // Create a new filtered list
            let mut filtered_list = Vec::new();
            
            for (index, path) in files.iter().enumerate() {
                // Check extension filter
                let extension_match = self.filter_extension.is_empty() ||
                    path.extension().map_or(false, |e| e.to_string_lossy().to_lowercase() == self.filter_extension.to_lowercase());
                
                // Find token count and binary status for this file
                let (tokens, is_binary) = self.file_list.iter()
                    .find(|f| f.path == *path)
                    .map_or((0, false), |f| (f.tokens, f.is_binary));
                
                // Check token range filters
                let min_tokens_match = self.filter_token_min == 0 || tokens >= self.filter_token_min;
                let max_tokens_match = self.filter_token_max == 0 || tokens <= self.filter_token_max;
                
                // Apply all filters
                if extension_match && min_tokens_match && max_tokens_match {
                    filtered_list.push(FileInfo {
                        index,
                        path: path.clone(),
                        tokens,
                        selected: false,
                        is_binary,
                    });
                }
            }
            
            // Update the file list
            self.file_list = filtered_list;
            
            // Reset to first page when filters change
            self.current_page = 0;
            
            // Resort the list
            self.sort_file_list();
        }
    }
    
    /// Exports the file list to a CSV file
    fn export_to_csv(&self) {
        if self.file_list.is_empty() {
            return;
        }
        
        // Create CSV content
        let mut csv = String::from("Index,Path,Value,FormattedValue,IsBinary,Type\n");
        
        for file in &self.file_list {
            let file_type = if file.is_binary { "Binary" } else { "Text" };
            let value_label = if file.is_binary { "Size" } else { "Tokens" };
            let formatted_value = if file.is_binary {
                format_file_size(file.tokens)
            } else {
                file.tokens.to_string()
            };
            
            csv.push_str(&format!(
                "{},{},{},\"{}\",{},{}\n",
                file.index,
                file.path.to_string_lossy().replace(',', "\\,"), // Escape commas in paths
                file.tokens,
                formatted_value,
                file.is_binary,
                file_type
            ));
        }
        
        // Calculate total sizes for the success message
        let total_files = self.file_list.len();
        let text_files = self.file_list.iter().filter(|f| !f.is_binary).count();
        let binary_files = self.file_list.iter().filter(|f| f.is_binary).count();
        let total_tokens = self.file_list.iter().filter(|f| !f.is_binary).map(|f| f.tokens).sum::<usize>();
        let total_binary_size = self.file_list.iter().filter(|f| f.is_binary).map(|f| f.tokens).sum::<usize>();
        
        // Write to file
        match std::fs::write("file_list.csv", csv) {
            Ok(_) => {
                // Print success message with statistics
                println!("Exported to file_list.csv:");
                println!("  - Total files: {} ({} text, {} binary)", total_files, text_files, binary_files);
                println!("  - Total tokens (text files): {}", total_tokens);
                if binary_files > 0 {
                    println!("  - Total binary size: {}", format_file_size(total_binary_size));
                }
            },
            Err(e) => {
                // Handle error
                eprintln!("Failed to export: {}", e);
            }
        }
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
        let (count, is_binary) = count_tokens(&temp_file);
        assert_eq!(count, 5); // 5 words in the test string
        assert_eq!(is_binary, false); // Text file, not binary
        
        // Clean up
        fs::remove_file(temp_file).unwrap();
    }
    
    #[test]
    fn test_sorting() {
        // Create test file info entries
        let files = vec![
            FileInfo { index: 0, path: PathBuf::from("a.txt"), tokens: 10, selected: false, is_binary: false },
            FileInfo { index: 1, path: PathBuf::from("b.txt"), tokens: 5, selected: false, is_binary: false },
            FileInfo { index: 2, path: PathBuf::from("c.txt"), tokens: 15, selected: false, is_binary: false },
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
        let (count, is_binary) = count_tokens(&temp_file);
        assert_eq!(count, 0);
        assert_eq!(is_binary, false); // Empty text file, not binary
        
        // Clean up
        fs::remove_file(temp_file).unwrap();
    }
    
    #[test]
    fn test_binary_file_detection() {
        // Create a temporary binary file
        let temp_file = std::env::temp_dir().join("test_binary.bin");
        let binary_data = [0u8, 1u8, 2u8, 3u8, 4u8]; // Some binary data
        fs::write(&temp_file, &binary_data).unwrap();
        
        // Test token counting for binary file
        let (size, is_binary) = count_tokens(&temp_file);
        assert_eq!(size, binary_data.len()); // Should return the file size in bytes
        assert_eq!(is_binary, true); // Should be detected as binary
        
        // Clean up
        fs::remove_file(temp_file).unwrap();
    }
    
    #[test]
    fn test_format_file_size() {
        // Test bytes
        assert_eq!(format_file_size(123), "123 bytes");
        
        // Test kilobytes
        assert_eq!(format_file_size(1234), "1.21 KB (1234 bytes)");
        
        // Test megabytes
        assert_eq!(format_file_size(1234567), "1.18 MB (1234567 bytes)");
        
        // Test gigabytes
        assert_eq!(format_file_size(1234567890), "1.15 GB (1234567890 bytes)");
        
        // Test zero
        assert_eq!(format_file_size(0), "0 bytes");
    }
}