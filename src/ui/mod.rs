use eframe::egui;

/// Handles UI components and interactions
pub struct UiHandler {
    /// Whether the UI is in a loading state
    is_loading: bool,
}

impl UiHandler {
    /// Creates a new UiHandler
    /// 
    /// # Returns
    /// A new UiHandler instance
    pub fn new() -> Self {
        Self {
            is_loading: false,
        }
    }
    
    /// Renders the top bar with URL input and controls
    /// 
    /// # Arguments
    /// * `ui` - The egui UI to render to
    /// * `git_url` - The Git URL input string
    /// * `keep_repository` - Whether to keep the repository after cloning
    /// * `on_clone_clicked` - Callback for when the Clone button is clicked
    /// 
    /// # Returns
    /// * `bool` - Whether the Clone button was clicked
    pub fn render_top_bar(
        &self,
        ui: &mut egui::Ui,
        git_url: &mut String,
        keep_repository: &mut bool,
    ) -> bool {
        let mut clone_clicked = false;
        
        ui.horizontal(|ui| {
            ui.label("Git URL:");
            
            // URL text field
            ui.text_edit_singleline(git_url);
            
            // Clone button
            let clone_button = ui.add_enabled(
                !self.is_loading && !git_url.is_empty(),
                egui::Button::new(
                    if self.is_loading { "Cloning..." } else { "Clone" }
                )
            );
            
            if clone_button.clicked() {
                clone_clicked = true;
            }
            
            // Keep repository checkbox
            ui.checkbox(keep_repository, "Keep Repository");
        });
        
        clone_clicked
    }
    
    /// Renders the status bar
    /// 
    /// # Arguments
    /// * `ui` - The egui UI to render to
    /// * `status_message` - The status message to display
    pub fn render_status_bar(&self, ui: &mut egui::Ui, status_message: &str) {
        ui.horizontal(|ui| {
            ui.label("Status:");
            
            // Status message with different styling based on loading state
            if self.is_loading {
                ui.colored_label(egui::Color32::from_rgb(100, 100, 250), status_message);
                
                // Add a spinner when loading
                ui.spinner();
            } else {
                ui.label(status_message);
            }
        });
    }
    
    /// Renders the empty state when no repository is loaded
    /// 
    /// # Arguments
    /// * `ui` - The egui UI to render to
    pub fn render_empty_state(&self, ui: &mut egui::Ui) {
        let available_size = ui.available_size();
        
        // Center the content
        ui.vertical_centered(|ui| {
            ui.add_space(available_size.y * 0.3); // Push down a bit
            
            ui.heading("Git Scroll");
            ui.add_space(10.0);
            
            ui.label("Enter a Git URL and click Clone to visualize the repository structure");
            
            // Example URL
            ui.add_space(20.0);
            ui.label("Example: https://github.com/username/repository.git");
        });
    }
    
    /// Sets the loading state
    /// 
    /// # Arguments
    /// * `loading` - Whether the UI is in a loading state
    pub fn set_loading(&mut self, loading: bool) {
        self.is_loading = loading;
    }
    
    /// Renders zoom controls
    /// 
    /// # Arguments
    /// * `ui` - The egui UI to render to
    /// 
    /// # Returns
    /// * `(bool, bool)` - Whether zoom in and zoom out buttons were clicked
    pub fn render_zoom_controls(&self, ui: &mut egui::Ui) -> (bool, bool) {
        let mut zoom_in_clicked = false;
        let mut zoom_out_clicked = false;
        
        ui.horizontal(|ui| {
            ui.label("Zoom:");
            
            if ui.button("−").clicked() {
                zoom_out_clicked = true;
            }
            
            if ui.button("+").clicked() {
                zoom_in_clicked = true;
            }
        });
        
        (zoom_in_clicked, zoom_out_clicked)
    }
    
    /// Renders a dropdown for layout selection
    /// 
    /// # Arguments
    /// * `ui` - The egui UI to render to
    /// * `current_layout` - The current layout type
    /// 
    /// # Returns
    /// * `Option<crate::visualization::LayoutType>` - The selected layout type, if changed
    pub fn render_layout_dropdown(
        &self,
        ui: &mut egui::Ui,
        current_layout: crate::visualization::LayoutType,
    ) -> Option<crate::visualization::LayoutType> {
        let mut selected_layout = current_layout;
        let mut layout_changed = false;
        
        ui.horizontal(|ui| {
            ui.label("Layout:");
            
            egui::ComboBox::new(egui::Id::new("layout_selector"), "")
                .selected_text(match current_layout {
                    crate::visualization::LayoutType::Grid => "Grid",
                    crate::visualization::LayoutType::Treemap => "Treemap",
                    crate::visualization::LayoutType::ForceDirected => "Force-Directed",
                    crate::visualization::LayoutType::Detailed => "Detailed",
                })
                .show_ui(ui, |ui| {
                    layout_changed |= ui.selectable_value(
                        &mut selected_layout, 
                        crate::visualization::LayoutType::Grid, 
                        "Grid"
                    ).clicked();
                    
                    layout_changed |= ui.selectable_value(
                        &mut selected_layout, 
                        crate::visualization::LayoutType::Treemap, 
                        "Treemap"
                    ).clicked();
                    
                    layout_changed |= ui.selectable_value(
                        &mut selected_layout, 
                        crate::visualization::LayoutType::ForceDirected, 
                        "Force-Directed"
                    ).clicked();
                    
                    layout_changed |= ui.selectable_value(
                        &mut selected_layout, 
                        crate::visualization::LayoutType::Detailed, 
                        "Detailed"
                    ).clicked();
                });
        });
        
        if layout_changed {
            Some(selected_layout)
        } else {
            None
        }
    }
    
    /// Renders a dropdown for theme selection
    /// 
    /// # Arguments
    /// * `ui` - The egui UI to render to
    /// * `current_theme` - The current theme
    /// 
    /// # Returns
    /// * `Option<crate::visualization::Theme>` - The selected theme, if changed
    pub fn render_theme_dropdown(
        &self,
        ui: &mut egui::Ui,
        current_theme: &crate::visualization::Theme,
    ) -> Option<crate::visualization::Theme> {
        let mut theme_changed = false;
        let mut selected_theme = current_theme.clone();
        
        ui.horizontal(|ui| {
            ui.label("Theme:");
            
            egui::ComboBox::new(egui::Id::new("theme_selector"), "")
                .selected_text(match current_theme {
                    crate::visualization::Theme::Light => "Light",
                    crate::visualization::Theme::Dark => "Dark",
                    crate::visualization::Theme::Custom(_) => "Custom",
                })
                .show_ui(ui, |ui| {
                    theme_changed |= ui.selectable_value(
                        &mut selected_theme, 
                        crate::visualization::Theme::Light, 
                        "Light"
                    ).clicked();
                    
                    theme_changed |= ui.selectable_value(
                        &mut selected_theme, 
                        crate::visualization::Theme::Dark, 
                        "Dark"
                    ).clicked();
                });
        });
        
        if theme_changed {
            Some(selected_theme)
        } else {
            None
        }
    }
    
    /// Renders a filter input field
    /// 
    /// # Arguments
    /// * `ui` - The egui UI to render to
    /// * `filter_pattern` - The current filter pattern
    /// 
    /// # Returns
    /// * `Option<String>` - The new filter pattern, if changed
    pub fn render_filter_input(
        &self,
        ui: &mut egui::Ui,
        filter_pattern: &str,
    ) -> Option<String> {
        let mut pattern = filter_pattern.to_string();
        let mut filter_changed = false;
        
        ui.horizontal(|ui| {
            ui.label("Filter:");
            filter_changed = ui.text_edit_singleline(&mut pattern).changed();
        });
        
        if filter_changed {
            Some(pattern)
        } else {
            None
        }
    }
}

/// Utility functions for UI styling
pub mod style {
    use eframe::egui;
    
    /// Applies custom styling to the UI
    /// 
    /// # Arguments
    /// * `ctx` - The egui context
    pub fn apply_style(ctx: &egui::Context) {
        let mut style = (*ctx.style()).clone();
        
        // Customize fonts
        let fonts = egui::FontDefinitions::default();
        
        // Customize colors
        style.visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(240, 240, 240);
        style.visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(220, 220, 220);
        style.visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(200, 200, 250);
        style.visuals.widgets.active.bg_fill = egui::Color32::from_rgb(180, 180, 250);
        
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
        let fonts = egui::FontDefinitions::default();
        
        // Customize colors for dark theme
        style.visuals.dark_mode = true;
        style.visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(40, 40, 40);
        style.visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(60, 60, 60);
        style.visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(80, 80, 120);
        style.visuals.widgets.active.bg_fill = egui::Color32::from_rgb(100, 100, 150);
        
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