use eframe::egui;
use crate::directory::DirectoryEntry;
use std::collections::HashMap;

/// Represents a visual square in the project area
pub struct VisualSquare {
    /// The directory entry this square represents
    pub entry: DirectoryEntry,
    
    /// Position and size of the square
    pub rect: egui::Rect,
    
    /// Whether this square is currently selected
    pub selected: bool,
    
    /// Current zoom level for this square
    pub zoom_level: ZoomLevel,
}

/// Represents different zoom levels for visualization
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ZoomLevel {
    /// Maximum zoom out - only directory squares visible
    MaxOut,
    
    /// Level 1 - Squares show files/subdirectories as rectangles
    Level1,
    
    /// Level 2 - Active square expands to show partial file content
    Level2,
    
    /// Maximum zoom in - Deepest directory level with all contents visible
    MaxIn,
}

/// Handles visualization of the directory structure
pub struct Visualizer {
    /// The root directory entry to visualize
    root_entry: Option<DirectoryEntry>,
    
    /// All visual squares in the project area
    squares: Vec<VisualSquare>,
    
    /// Currently selected square index
    selected_index: Option<usize>,
    
    /// Current global zoom level
    zoom_level: ZoomLevel,
    
    /// Cache of colors for different file types
    file_type_colors: HashMap<String, egui::Color32>,
}

impl Visualizer {
    /// Creates a new Visualizer
    /// 
    /// # Returns
    /// A new Visualizer instance
    pub fn new() -> Self {
        Self {
            root_entry: None,
            squares: Vec::new(),
            selected_index: None,
            zoom_level: ZoomLevel::MaxOut,
            file_type_colors: Self::initialize_file_colors(),
        }
    }
    
    /// Sets the root directory entry to visualize
    /// 
    /// # Arguments
    /// * `entry` - The root directory entry
    pub fn set_root_entry(&mut self, entry: DirectoryEntry) {
        self.root_entry = Some(entry);
        self.generate_squares();
    }
    
    /// Generates visual squares from the directory structure
    fn generate_squares(&mut self) {
        self.squares.clear();
        self.selected_index = None;
        
        if let Some(root) = &self.root_entry {
            // Start with the top-level directories
            for (index, child) in root.children.iter().enumerate() {
                if child.is_directory {
                    // For now, just create placeholder squares
                    // In a real implementation, we would calculate positions based on layout
                    let rect = egui::Rect::from_min_size(
                        egui::pos2(100.0 * index as f32, 100.0),
                        egui::vec2(80.0, 80.0),
                    );
                    
                    self.squares.push(VisualSquare {
                        entry: child.clone(),
                        rect,
                        selected: false,
                        zoom_level: self.zoom_level,
                    });
                }
            }
        }
    }
    
    /// Handles mouse interaction with the visualization
    /// 
    /// # Arguments
    /// * `ui` - The egui UI to interact with
    /// * `pointer_pos` - The current pointer position
    /// * `clicked` - Whether the mouse was clicked
    pub fn handle_interaction(&mut self, ui: &mut egui::Ui, pointer_pos: Option<egui::Pos2>, clicked: bool) {
        if let Some(pos) = pointer_pos {
            // Check if hovering over any square
            for (index, square) in self.squares.iter_mut().enumerate() {
                if square.rect.contains(pos) {
                    // Show tooltip on hover
                    ui.ctx().tooltip_text(pos, &square.entry.name);
                    
                    // Handle click
                    if clicked {
                        // Deselect previously selected square
                        if let Some(prev_index) = self.selected_index {
                            if prev_index != index {
                                self.squares[prev_index].selected = false;
                            }
                        }
                        
                        // Select this square
                        square.selected = true;
                        self.selected_index = Some(index);
                    }
                }
            }
        }
    }
    
    /// Renders the visualization
    /// 
    /// # Arguments
    /// * `ui` - The egui UI to render to
    pub fn render(&self, ui: &mut egui::Ui) {
        let canvas_rect = ui.available_rect_before_wrap();
        
        // Draw the canvas background
        ui.painter().rect_filled(
            canvas_rect,
            0.0,
            egui::Color32::from_rgb(240, 240, 240),
        );
        
        // Draw each square
        for square in &self.squares {
            // Choose color based on selection state
            let fill_color = if square.selected {
                egui::Color32::from_rgb(100, 150, 250) // Blue for selected
            } else {
                egui::Color32::from_rgb(70, 130, 180) // Steel blue for directories
            };
            
            // Draw the square
            ui.painter().rect_filled(
                square.rect,
                4.0, // Rounded corners
                fill_color,
            );
            
            // Draw the square border
            ui.painter().rect_stroke(
                square.rect,
                4.0, // Rounded corners
                egui::Stroke::new(1.0, egui::Color32::BLACK),
            );
            
            // Draw the directory name
            let text_pos = square.rect.center();
            ui.painter().text(
                text_pos,
                egui::Align2::CENTER_CENTER,
                &square.entry.name,
                egui::FontId::proportional(14.0),
                egui::Color32::WHITE,
            );
        }
    }
    
    /// Initializes the color map for different file types
    /// 
    /// # Returns
    /// A HashMap mapping file extensions to colors
    fn initialize_file_colors() -> HashMap<String, egui::Color32> {
        let mut colors = HashMap::new();
        
        // Add colors for common file types
        colors.insert("rs".to_string(), egui::Color32::from_rgb(250, 100, 100)); // Rust files
        colors.insert("js".to_string(), egui::Color32::from_rgb(240, 220, 100)); // JavaScript
        colors.insert("py".to_string(), egui::Color32::from_rgb(100, 200, 150)); // Python
        colors.insert("md".to_string(), egui::Color32::from_rgb(150, 150, 250)); // Markdown
        colors.insert("txt".to_string(), egui::Color32::from_rgb(200, 200, 200)); // Text
        colors.insert("json".to_string(), egui::Color32::from_rgb(250, 150, 100)); // JSON
        
        colors
    }
    
    /// Gets the color for a file based on its extension
    /// 
    /// # Arguments
    /// * `file_name` - The name of the file
    /// 
    /// # Returns
    /// The color for the file type
    fn get_file_color(&self, file_name: &str) -> egui::Color32 {
        if let Some(extension) = file_name.split('.').last() {
            if let Some(color) = self.file_type_colors.get(extension) {
                return *color;
            }
        }
        
        // Default color for unknown file types
        egui::Color32::from_rgb(180, 180, 180)
    }
    
    /// Zooms the visualization in or out
    /// 
    /// # Arguments
    /// * `zoom_in` - Whether to zoom in (true) or out (false)
    pub fn zoom(&mut self, zoom_in: bool) {
        self.zoom_level = match (self.zoom_level, zoom_in) {
            (ZoomLevel::MaxOut, true) => ZoomLevel::Level1,
            (ZoomLevel::Level1, true) => ZoomLevel::Level2,
            (ZoomLevel::Level2, true) => ZoomLevel::MaxIn,
            (ZoomLevel::MaxIn, false) => ZoomLevel::Level2,
            (ZoomLevel::Level2, false) => ZoomLevel::Level1,
            (ZoomLevel::Level1, false) => ZoomLevel::MaxOut,
            _ => self.zoom_level, // No change at the limits
        };
        
        // Update all squares with the new zoom level
        for square in &mut self.squares {
            square.zoom_level = self.zoom_level;
        }
        
        // Regenerate the visualization based on the new zoom level
        if let Some(root) = &self.root_entry {
            self.set_root_entry(root.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    
    #[test]
    fn test_file_colors() {
        let visualizer = Visualizer::new();
        
        // Test known file types
        let rust_color = visualizer.get_file_color("main.rs");
        let js_color = visualizer.get_file_color("script.js");
        
        // These should be different colors
        assert_ne!(rust_color, js_color);
        
        // Test unknown file type
        let unknown_color = visualizer.get_file_color("file.xyz");
        assert_eq!(unknown_color, egui::Color32::from_rgb(180, 180, 180));
    }
    
    #[test]
    fn test_zoom_levels() {
        let mut visualizer = Visualizer::new();
        
        // Start at MaxOut
        assert_eq!(visualizer.zoom_level, ZoomLevel::MaxOut);
        
        // Zoom in once
        visualizer.zoom(true);
        assert_eq!(visualizer.zoom_level, ZoomLevel::Level1);
        
        // Zoom in again
        visualizer.zoom(true);
        assert_eq!(visualizer.zoom_level, ZoomLevel::Level2);
        
        // Zoom in to max
        visualizer.zoom(true);
        assert_eq!(visualizer.zoom_level, ZoomLevel::MaxIn);
        
        // Try to zoom in beyond max (should stay at MaxIn)
        visualizer.zoom(true);
        assert_eq!(visualizer.zoom_level, ZoomLevel::MaxIn);
        
        // Zoom out
        visualizer.zoom(false);
        assert_eq!(visualizer.zoom_level, ZoomLevel::Level2);
    }
}