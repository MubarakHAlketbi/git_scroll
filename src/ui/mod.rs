use eframe::egui;
use std::time::Duration;
use egui::LayerId;

/// Handles UI components and interactions
pub struct UiHandler {
    /// Whether the UI is in a loading state
    is_loading: bool,
    /// Progress value for operations (0.0 to 1.0)
    progress: f32,
    /// Whether to use dark mode
    dark_mode: bool,
}

impl UiHandler {
    /// Creates a new UiHandler
    ///
    /// # Returns
    /// A new UiHandler instance
    pub fn new() -> Self {
        Self {
            is_loading: false,
            progress: 0.0,
            dark_mode: true, // Default to dark mode
        }
    }
    
    /// Renders the top bar with URL input and controls
    ///
    /// # Arguments
    /// * `ui` - The egui UI to render to
    /// * `git_url` - The Git URL input string
    /// * `keep_repository` - Whether to keep the repository after cloning
    ///
    /// # Returns
    /// * `(bool, bool)` - Whether the Clone button was clicked and whether the Clear button was clicked
    pub fn render_top_bar(
        &self,
        ui: &mut egui::Ui,
        git_url: &mut String,
        keep_repository: &mut bool,
    ) -> (bool, bool) {
        let mut clone_clicked = false;
        let mut clear_clicked = false;
        
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Git URL:").strong());
            
            // URL text field with placeholder
            let _response = ui.add(
                egui::TextEdit::singleline(git_url)
                    .hint_text("Enter repository URL...")
                    .desired_width(ui.available_width() - 250.0)
            );
            
            // Clone button with improved styling
            let clone_button = ui.add_enabled(
                !self.is_loading && !git_url.is_empty(),
                egui::Button::new(
                    egui::RichText::new(if self.is_loading { "Cloning..." } else { "Clone" })
                    .strong()
                )
                .min_size(egui::vec2(80.0, 28.0))
            );
            
            if clone_button.clicked() {
                clone_clicked = true;
            }
            
            // Add tooltip to Clone button
            if clone_button.hovered() {
                egui::show_tooltip(ui.ctx(), LayerId::background(), egui::Id::new("clone_tooltip"), |ui| {
                    ui.label("Clone the repository and analyze its structure");
                });
            }
            
            // Keep repository checkbox
            ui.checkbox(keep_repository, "Keep Repository");
            
            // Clear button
            let clear_button = ui.add(
                egui::Button::new(egui::RichText::new("Clear").strong())
                .min_size(egui::vec2(60.0, 28.0))
            );
            
            if clear_button.clicked() {
                clear_clicked = true;
            }
            
            // Add tooltip to Clear button
            if clear_button.hovered() {
                egui::show_tooltip(ui.ctx(), LayerId::background(), egui::Id::new("clear_tooltip"), |ui| {
                    ui.label("Clear the current repository and start over");
                });
            }
            
            // Theme toggle
            if ui.add(egui::Button::new(
                if self.dark_mode { "☀" } else { "🌙" }
            )).clicked() {
                // Toggle handled in app.rs
            }
        });
        
        (clone_clicked, clear_clicked)
    }
    
    /// Renders the controls bar with sorting and filtering options
    ///
    /// # Arguments
    /// * `ui` - The egui UI to render to
    /// * `sort_column` - The current sort column
    /// * `sort_direction` - The current sort direction
    /// * `filter_pattern` - The current filter pattern
    ///
    /// # Returns
    /// * `bool` - Whether any control was changed
    pub fn render_controls_bar(
        &self,
        ui: &mut egui::Ui,
        sort_column: &mut super::app::SortColumn,
        sort_direction: &mut super::app::SortDirection,
        filter_pattern: &mut String,
    ) -> bool {
        let mut changed = false;
        
        ui.horizontal(|ui| {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Sort:").strong());
                    
                    if ui.selectable_label(
                        *sort_column == super::app::SortColumn::Index,
                        "Index"
                    ).clicked() {
                        *sort_column = super::app::SortColumn::Index;
                        changed = true;
                    }
                    
                    if ui.selectable_label(
                        *sort_column == super::app::SortColumn::Name,
                        "Name"
                    ).clicked() {
                        *sort_column = super::app::SortColumn::Name;
                        changed = true;
                    }
                    
                    if ui.selectable_label(
                        *sort_column == super::app::SortColumn::Tokens,
                        "Tokens"
                    ).clicked() {
                        *sort_column = super::app::SortColumn::Tokens;
                        changed = true;
                    }
                });
            });
            
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Direction:").strong());
                    
                    if ui.selectable_label(
                        *sort_direction == super::app::SortDirection::Ascending,
                        "↑ Asc"
                    ).clicked() {
                        *sort_direction = super::app::SortDirection::Ascending;
                        changed = true;
                    }
                    
                    if ui.selectable_label(
                        *sort_direction == super::app::SortDirection::Descending,
                        "↓ Desc"
                    ).clicked() {
                        *sort_direction = super::app::SortDirection::Descending;
                        changed = true;
                    }
                });
            });
            
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Filter:").strong());
                    
                    // Filter input with debouncing
                    let response = ui.add(
                        egui::TextEdit::singleline(filter_pattern)
                            .hint_text("Filter files...")
                            .desired_width(200.0)
                    );
                    
                    if response.changed() {
                        // Request repaint after a short delay for debouncing
                        ui.ctx().request_repaint_after(Duration::from_millis(300));
                        changed = true;
                    }
                });
            });
        });
        
        changed
    }
    
    /// Renders the status bar
    ///
    /// # Arguments
    /// * `ui` - The egui UI to render to
    /// * `status_message` - The status message to display
    pub fn render_status_bar(&self, ui: &mut egui::Ui, status_message: &str) {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Status:").strong());
            
            // Status message with different styling based on loading state
            if self.is_loading {
                ui.colored_label(egui::Color32::from_rgb(100, 150, 255), status_message);
                
                // Add a progress bar when loading
                if self.progress > 0.0 {
                    ui.add(egui::ProgressBar::new(self.progress)
                        .desired_width(100.0)
                        .animate(true)
                    );
                } else {
                    // Use spinner when progress is indeterminate
                    ui.spinner();
                }
            } else if status_message.contains("error") || status_message.contains("fail") {
                // Red for errors
                ui.colored_label(egui::Color32::from_rgb(255, 100, 100), status_message);
            } else if status_message.contains("success") {
                // Green for success
                ui.colored_label(egui::Color32::from_rgb(100, 200, 100), status_message);
            } else {
                ui.label(status_message);
            }
        });
    }
    
    /// Renders the stats bar at the bottom of the screen
    ///
    /// # Arguments
    /// * `ui` - The egui UI to render to
    /// * `file_list` - The list of files to show stats for
    pub fn render_stats_bar(&self, ui: &mut egui::Ui, file_list: &[super::app::FileInfo]) {
        if file_list.is_empty() {
            return;
        }
        
        let total_files = file_list.len();
        let total_tokens = file_list.iter().map(|f| f.tokens).sum::<usize>();
        let avg_tokens = if total_files > 0 { total_tokens / total_files } else { 0 };
        
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(format!("Files: {}", total_files)).strong());
            ui.separator();
            ui.label(egui::RichText::new(format!("Total Tokens: {}", total_tokens)).strong());
            ui.separator();
            ui.label(format!("Avg: {}", avg_tokens));
            
            // Add top files button
            if ui.button("Top Files").clicked() {
                // Handled in app.rs
            }
        });
    }
    
    /// Renders the empty state when no repository is loaded
    ///
    /// # Arguments
    /// * `ui` - The egui UI to render to
    /// * `git_url` - The Git URL input string to potentially update
    pub fn render_empty_state(&self, ui: &mut egui::Ui, git_url: &mut String) {
        let available_size = ui.available_size();
        
        // Center the content
        ui.vertical_centered(|ui| {
            ui.add_space(available_size.y * 0.2); // Push down a bit
            
            // Add a card-like container
            egui::Frame::group(ui.style())
                .fill(if self.dark_mode {
                    egui::Color32::from_rgb(45, 45, 48)
                } else {
                    egui::Color32::from_rgb(240, 240, 245)
                })
                .corner_radius(10.0)
                .shadow(egui::epaint::Shadow {
                    offset: [0, 0],
                    blur: 5,
                    spread: 0,
                    color: egui::Color32::from_black_alpha(40),
                })
                .show(ui, |ui| {
                    ui.set_width(400.0);
                    ui.vertical_centered(|ui| {
                        ui.add_space(20.0);
                        
                        // Icon (using text as placeholder)
                        ui.label(egui::RichText::new("📊").size(40.0));
                        ui.add_space(10.0);
                        
                        // Title
                        ui.heading(egui::RichText::new("Analyze Your Repository").size(24.0));
                        ui.add_space(10.0);
                        
                        // Description
                        ui.label("Paste a Git URL to see file details and token counts");
                        ui.add_space(20.0);
                        
                        // Example URLs as clickable links
                        ui.label(egui::RichText::new("Try these examples:").strong());
                        ui.add_space(5.0);
                        
                        if ui.link(egui::RichText::new("https://github.com/rust-lang/rust-analyzer.git")).clicked() {
                            *git_url = "https://github.com/rust-lang/rust-analyzer.git".to_string();
                        }
                        
                        if ui.link(egui::RichText::new("https://github.com/emilk/egui.git")).clicked() {
                            *git_url = "https://github.com/emilk/egui.git".to_string();
                        }
                        
                        ui.add_space(20.0);
                    });
                });
        });
    }
    
    /// Sets the loading state
    ///
    /// # Arguments
    /// * `loading` - Whether the UI is in a loading state
    pub fn set_loading(&mut self, loading: bool) {
        self.is_loading = loading;
        if !loading {
            self.progress = 0.0;
        }
    }
    
    /// Sets the progress value
    ///
    /// # Arguments
    /// * `progress` - Progress value between 0.0 and 1.0
    pub fn set_progress(&mut self, progress: f32) {
        self.progress = progress.clamp(0.0, 1.0);
    }
    
    /// Sets the dark mode state
    ///
    /// # Arguments
    /// * `dark_mode` - Whether to use dark mode
    pub fn set_dark_mode(&mut self, dark_mode: bool) {
        self.dark_mode = dark_mode;
    }
    
    /// Gets the dark mode state
    ///
    /// # Returns
    /// * `bool` - Whether dark mode is enabled
    pub fn is_dark_mode(&self) -> bool {
        self.dark_mode
    }
    
    /// Truncates a path with ellipsis for display
    ///
    /// # Arguments
    /// * `path_str` - The path string to truncate
    /// * `max_length` - The maximum length before truncation
    ///
    /// # Returns
    /// * `String` - The truncated path string
    pub fn truncate_path(path_str: &str, max_length: usize) -> String {
        if path_str.len() <= max_length {
            return path_str.to_string();
        }
        
        // Find a good place to truncate
        let parts: Vec<&str> = path_str.split('/').collect();
        if parts.len() <= 2 {
            // If there are only 1-2 parts, just truncate with ellipsis
            return format!("{}...", &path_str[0..max_length - 3]);
        }
        
        // Keep the first and last parts, truncate the middle
        let first = parts.first().unwrap_or(&"");
        let last = parts.last().unwrap_or(&"");
        
        if first.len() + last.len() + 5 > max_length {
            // If first+last is already too long, truncate the last part
            return format!("{}...{}", first, &last[last.len().saturating_sub(max_length - first.len() - 5)..]);
        }
        
        format!("{}/.../{}", first, last)
    }
}

/// Utility functions for UI styling
pub mod style {
    use eframe::egui;
    
    /// Applies custom styling to the UI
    ///
    /// # Arguments
    /// * `ctx` - The egui context
    /// * `dark_mode` - Whether to use dark mode
    pub fn apply_style(ctx: &egui::Context, dark_mode: bool) {
        if dark_mode {
            apply_dark_style(ctx);
        } else {
            apply_light_style(ctx);
        }
    }
    
    /// Applies light theme styling to the UI
    ///
    /// # Arguments
    /// * `ctx` - The egui context
    pub fn apply_light_style(ctx: &egui::Context) {
        let mut style = (*ctx.style()).clone();
        
        // Customize fonts
        let mut fonts = egui::FontDefinitions::default();
        
        // Add monospace font for code-related data
        fonts.font_data.insert(
            "jetbrains_mono".to_owned(),
            std::sync::Arc::new(egui::FontData::from_static(include_bytes!("../../assets/JetBrainsMono-Regular.ttf"))),
        );
        
        // Set monospace as the proportional font for code
        fonts.families
            .entry(egui::FontFamily::Monospace)
            .or_default()
            .insert(0, "jetbrains_mono".to_owned());
        
        // Customize colors for light theme
        style.visuals.dark_mode = false;
        style.visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(245, 245, 245);
        style.visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(230, 230, 230);
        style.visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(220, 220, 250);
        style.visuals.widgets.active.bg_fill = egui::Color32::from_rgb(200, 200, 250);
        
        // Customize spacing
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(6.0, 4.0);
        
        // Apply the style
        ctx.set_style(style);
        ctx.set_fonts(fonts);
    }
    
    /// Applies dark theme styling to the UI
    ///
    /// # Arguments
    /// * `ctx` - The egui context
    pub fn apply_dark_style(ctx: &egui::Context) {
        let mut style = (*ctx.style()).clone();
        
        // Customize fonts
        let mut fonts = egui::FontDefinitions::default();
        
        // Add monospace font for code-related data
        fonts.font_data.insert(
            "jetbrains_mono".to_owned(),
            std::sync::Arc::new(egui::FontData::from_static(include_bytes!("../../assets/JetBrainsMono-Regular.ttf"))),
        );
        
        // Set monospace as the proportional font for code
        fonts.families
            .entry(egui::FontFamily::Monospace)
            .or_default()
            .insert(0, "jetbrains_mono".to_owned());
        
        // Customize colors for dark theme
        style.visuals.dark_mode = true;
        style.visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(42, 42, 42);
        style.visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(50, 50, 50);
        style.visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(60, 60, 80);
        style.visuals.widgets.active.bg_fill = egui::Color32::from_rgb(70, 70, 100);
        
        // Customize text colors for better contrast
        style.visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(220, 220, 220));
        
        // Customize spacing
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(6.0, 4.0);
        
        // Apply the style
        ctx.set_style(style);
        ctx.set_fonts(fonts);
    }
    
    /// Gets the color for a directory
    ///
    /// # Returns
    /// * `egui::Color32` - The color for directories
    pub fn directory_color() -> egui::Color32 {
        egui::Color32::from_rgb(70, 130, 180) // Steel blue
    }
    
    /// Gets the color for a file
    ///
    /// # Returns
    /// * `egui::Color32` - The color for files
    pub fn file_color() -> egui::Color32 {
        egui::Color32::from_rgb(100, 180, 100) // Green
    }
    
    /// Gets the color for selected items
    ///
    /// # Returns
    /// * `egui::Color32` - The color for selected items
    pub fn selected_color() -> egui::Color32 {
        egui::Color32::from_rgb(100, 150, 250) // Blue
    }
    
    /// Gets the header background color
    ///
    /// # Arguments
    /// * `dark_mode` - Whether dark mode is enabled
    ///
    /// # Returns
    /// * `egui::Color32` - The color for table headers
    pub fn header_color(dark_mode: bool) -> egui::Color32 {
        if dark_mode {
            egui::Color32::from_rgb(60, 60, 80)
        } else {
            egui::Color32::from_rgb(220, 220, 240)
        }
    }
    
    /// Gets the alternating row colors
    ///
    /// # Arguments
    /// * `dark_mode` - Whether dark mode is enabled
    ///
    /// # Returns
    /// * `(egui::Color32, egui::Color32)` - The colors for even and odd rows
    pub fn row_colors(dark_mode: bool) -> (egui::Color32, egui::Color32) {
        if dark_mode {
            (
                egui::Color32::from_rgb(45, 45, 45),
                egui::Color32::from_rgb(50, 50, 50)
            )
        } else {
            (
                egui::Color32::from_rgb(245, 245, 245),
                egui::Color32::from_rgb(235, 235, 235)
            )
        }
    }
    
    /// Gets the token count background color based on value
    ///
    /// # Arguments
    /// * `tokens` - The token count
    /// * `max_tokens` - The maximum token count for scaling
    /// * `dark_mode` - Whether dark mode is enabled
    ///
    /// # Returns
    /// * `egui::Color32` - The color for the token count background
    pub fn token_count_color(tokens: usize, max_tokens: usize, dark_mode: bool) -> egui::Color32 {
        let ratio = if max_tokens > 0 {
            (tokens as f32 / max_tokens as f32).clamp(0.0, 1.0)
        } else {
            0.0
        };
        
        if dark_mode {
            let r = (70.0 + ratio * 50.0) as u8;
            let g = (100.0 + ratio * 50.0) as u8;
            let b = (130.0 - ratio * 30.0) as u8;
            egui::Color32::from_rgb(r, g, b)
        } else {
            let r = (220.0 + ratio * 35.0) as u8;
            let g = (240.0 - ratio * 40.0) as u8;
            let b = (255.0 - ratio * 55.0) as u8;
            egui::Color32::from_rgb(r, g, b)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_ui_handler_creation() {
        let handler = UiHandler::new();
        assert!(!handler.is_loading);
        assert_eq!(handler.progress, 0.0);
        assert!(handler.dark_mode); // Default to dark mode
    }
    
    #[test]
    fn test_loading_state() {
        let mut handler = UiHandler::new();
        
        // Initially not loading
        assert!(!handler.is_loading);
        
        // Set to loading
        handler.set_loading(true);
        assert!(handler.is_loading);
        
        // Set back to not loading
        handler.set_loading(false);
        assert!(!handler.is_loading);
        assert_eq!(handler.progress, 0.0);
    }
    
    #[test]
    fn test_progress_value() {
        let mut handler = UiHandler::new();
        
        // Set progress to 50%
        handler.set_progress(0.5);
        assert_eq!(handler.progress, 0.5);
        
        // Ensure progress is clamped
        handler.set_progress(1.5);
        assert_eq!(handler.progress, 1.0);
        
        handler.set_progress(-0.5);
        assert_eq!(handler.progress, 0.0);
    }
    
    #[test]
    fn test_dark_mode() {
        let mut handler = UiHandler::new();
        
        // Default is dark mode
        assert!(handler.is_dark_mode());
        
        // Toggle to light mode
        handler.set_dark_mode(false);
        assert!(!handler.is_dark_mode());
        
        // Toggle back to dark mode
        handler.set_dark_mode(true);
        assert!(handler.is_dark_mode());
    }
    
    #[test]
    fn test_truncate_path() {
        // Short path, no truncation
        assert_eq!(
            UiHandler::truncate_path("file.txt", 20),
            "file.txt"
        );
        
        // Simple truncation
        assert_eq!(
            UiHandler::truncate_path("this_is_a_very_long_filename.txt", 20),
            "this_is_a_very_lon..."
        );
        
        // Path with multiple segments
        assert_eq!(
            UiHandler::truncate_path("src/module/file.txt", 20),
            "src/.../file.txt"
        );
        
        // Path with very long segments
        assert_eq!(
            UiHandler::truncate_path("very_long_directory/another_long_name/file.txt", 20),
            "very_long_directory/...file.txt"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    // Note: UI tests are typically difficult to write as unit tests
    // since they depend on rendering context. These tests are minimal.
    
    #[test]
    fn test_ui_handler_creation() {
        let handler = UiHandler::new();
        assert!(!handler.is_loading);
    }
    
    #[test]
    fn test_loading_state() {
        let mut handler = UiHandler::new();
        
        // Initially not loading
        assert!(!handler.is_loading);
        
        // Set to loading
        handler.set_loading(true);
        assert!(handler.is_loading);
        
        // Set back to not loading
        handler.set_loading(false);
        assert!(!handler.is_loading);
    }
}