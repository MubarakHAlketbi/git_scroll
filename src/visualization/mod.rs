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
    
    /// Currently hovered square index
    hovered_index: Option<usize>,
    
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
            hovered_index: None,
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
            // Calculate the available canvas size
            // For now, we'll use a fixed size, but this could be made dynamic
            let canvas_width: f32 = 800.0;
            let canvas_height: f32 = 600.0;
            
            // Clone the root to avoid borrowing issues
            let root_clone = root.clone();
            
            // Create the layout based on zoom level
            match self.zoom_level {
                ZoomLevel::MaxOut => {
                    // At MaxOut, only show top-level directories in a grid layout
                    self.generate_grid_layout(&root_clone, canvas_width, canvas_height, 0);
                },
                ZoomLevel::Level1 => {
                    // At Level1, show directories and files with different sizes
                    self.generate_treemap_layout(&root_clone, canvas_width, canvas_height);
                },
                ZoomLevel::Level2 | ZoomLevel::MaxIn => {
                    // At deeper zoom levels, show more detail
                    self.generate_detailed_layout(&root_clone, canvas_width, canvas_height);
                }
            }
        }
    }
    
    /// Generates a grid layout for directories
    ///
    /// # Arguments
    /// * `entry` - The directory entry to visualize
    /// * `width` - Available width
    /// * `height` - Available height
    /// * `depth` - Current depth in the directory tree
    fn generate_grid_layout(&mut self, entry: &DirectoryEntry, width: f32, height: f32, depth: usize) {
        // Only process if this is a directory
        if !entry.is_directory {
            return;
        }
        
        // Filter to only include directories
        let directories: Vec<&DirectoryEntry> = entry.children.iter()
            .filter(|child| child.is_directory)
            .collect();
        
        let dir_count = directories.len();
        if dir_count == 0 {
            return;
        }
        
        // Calculate grid dimensions
        let cols = (dir_count as f32).sqrt().ceil() as usize;
        let rows = (dir_count + cols - 1) / cols; // Ceiling division
        
        // Calculate cell size
        let cell_width = width / cols as f32;
        let cell_height = height / rows as f32;
        
        // Create squares for each directory
        for (index, dir) in directories.iter().enumerate() {
            let row = index / cols;
            let col = index % cols;
            
            let x = col as f32 * cell_width;
            let y = row as f32 * cell_height;
            
            // Add padding
            let padding = 10.0;
            let rect = egui::Rect::from_min_size(
                egui::pos2(x + padding, y + padding),
                egui::vec2(cell_width - 2.0 * padding, cell_height - 2.0 * padding),
            );
            
            // Clone the directory entry to get an owned copy
            let dir_entry = (*dir).clone();
            
            self.squares.push(VisualSquare {
                entry: dir_entry,
                rect,
                selected: false,
                zoom_level: self.zoom_level,
            });
        }
    }
    
    /// Generates a treemap layout that represents directory sizes
    ///
    /// # Arguments
    /// * `entry` - The directory entry to visualize
    /// * `width` - Available width
    /// * `height` - Available height
    fn generate_treemap_layout(&mut self, entry: &DirectoryEntry, width: f32, height: f32) {
        // Only process if this is a directory
        if !entry.is_directory {
            return;
        }
        
        // Get all children (both directories and files)
        let children = &entry.children;
        
        if children.is_empty() {
            return;
        }
        
        // Calculate total size (use child count as a proxy for size if needed)
        let total_size: usize = children.len();
        
        // Sort children by size (directories first, then files)
        let mut sorted_children: Vec<&DirectoryEntry> = children.iter().collect();
        sorted_children.sort_by(|a, b| {
            // Directories come before files
            if a.is_directory && !b.is_directory {
                std::cmp::Ordering::Less
            } else if !a.is_directory && b.is_directory {
                std::cmp::Ordering::Greater
            } else {
                // Within same type, sort by number of children (for directories) or alphabetically (for files)
                if a.is_directory {
                    b.children.len().cmp(&a.children.len())
                } else {
                    a.name.cmp(&b.name)
                }
            }
        });
        
        // Use a simple algorithm to divide the space
        let mut x = 0.0;
        let mut y = 0.0;
        let mut row_height: f32 = 0.0;
        let mut remaining_width = width;
        
        for child in sorted_children {
            // Calculate size proportion
            let size_proportion = 1.0 / total_size as f32;
            let item_area = width * height * size_proportion;
            
            // Calculate dimensions
            let item_width = if remaining_width > 0.0 {
                (item_area / height).min(remaining_width)
            } else {
                item_area / height
            };
            let item_height = item_area / item_width;
            
            // Check if we need to start a new row
            if x + item_width > width && x > 0.0 {
                x = 0.0;
                y += row_height;
                row_height = 0.0;
                remaining_width = width;
            }
            
            // Update row height
            row_height = row_height.max(item_height);
            
            // Create the rectangle
            let padding = 5.0;
            let rect = egui::Rect::from_min_size(
                egui::pos2(x + padding, y + padding),
                egui::vec2(item_width - 2.0 * padding, item_height - 2.0 * padding),
            );
            
            // Clone the directory entry to get an owned copy
            let child_entry = (*child).clone();
            
            // Add the square
            self.squares.push(VisualSquare {
                entry: child_entry,
                rect,
                selected: false,
                zoom_level: self.zoom_level,
            });
            
            // Update position for next item
            x += item_width;
            remaining_width -= item_width;
        }
    }
    
    /// Generates a detailed layout showing files and directories
    ///
    /// # Arguments
    /// * `entry` - The directory entry to visualize
    /// * `width` - Available width
    /// * `height` - Available height
    fn generate_detailed_layout(&mut self, entry: &DirectoryEntry, width: f32, height: f32) {
        // Only process if this is a directory
        if !entry.is_directory {
            return;
        }
        
        // Get all children (both directories and files)
        let children = &entry.children;
        
        if children.is_empty() {
            return;
        }
        
        // Separate directories and files
        let directories: Vec<&DirectoryEntry> = children.iter()
            .filter(|child| child.is_directory)
            .collect();
        
        let files: Vec<&DirectoryEntry> = children.iter()
            .filter(|child| !child.is_directory)
            .collect();
        
        // Allocate space: 70% for directories, 30% for files
        let dir_height = if !directories.is_empty() { height * 0.7 } else { 0.0 };
        let file_height = if !files.is_empty() { height - dir_height } else { 0.0 };
        
        // Generate layout for directories (if any)
        if !directories.is_empty() {
            let dir_count = directories.len();
            let cols = (dir_count as f32).sqrt().ceil() as usize;
            let rows = (dir_count + cols - 1) / cols;
            
            let cell_width = width / cols as f32;
            let cell_height = dir_height / rows as f32;
            
            for (index, dir) in directories.iter().enumerate() {
                let row = index / cols;
                let col = index % cols;
                
                let x = col as f32 * cell_width;
                let y = row as f32 * cell_height;
                
                let padding = 8.0;
                let rect = egui::Rect::from_min_size(
                    egui::pos2(x + padding, y + padding),
                    egui::vec2(cell_width - 2.0 * padding, cell_height - 2.0 * padding),
                );
                
                // Clone the directory entry to get an owned copy
                let dir_entry = (*dir).clone();
                
                self.squares.push(VisualSquare {
                    entry: dir_entry,
                    rect,
                    selected: false,
                    zoom_level: self.zoom_level,
                });
            }
        }
        
        // Generate layout for files (if any)
        if !files.is_empty() {
            let file_count = files.len();
            let file_cols = (file_count as f32).sqrt().ceil() as usize;
            let file_rows = (file_count + file_cols - 1) / file_cols;
            
            let file_cell_width = width / file_cols as f32;
            let file_cell_height = file_height / file_rows as f32;
            
            for (index, file) in files.iter().enumerate() {
                let row = index / file_cols;
                let col = index % file_cols;
                
                let x = col as f32 * file_cell_width;
                let y = dir_height + row as f32 * file_cell_height;
                
                let padding = 5.0;
                let rect = egui::Rect::from_min_size(
                    egui::pos2(x + padding, y + padding),
                    egui::vec2(file_cell_width - 2.0 * padding, file_cell_height - 2.0 * padding),
                );
                
                // Clone the file entry to get an owned copy
                let file_entry = (*file).clone();
                
                self.squares.push(VisualSquare {
                    entry: file_entry,
                    rect,
                    selected: false,
                    zoom_level: self.zoom_level,
                });
            }
        }
    }
    
    /// Handles mouse interaction with the visualization
    ///
    /// # Arguments
    /// * `ui` - The egui UI to interact with
    /// * `pointer_pos` - The current pointer position
    /// * `clicked` - Whether the mouse was clicked
    pub fn handle_interaction(&mut self, _ui: &mut egui::Ui, pointer_pos: Option<egui::Pos2>, clicked: bool) {
        // Reset hover state
        self.hovered_index = None;
        
        // First, find the square that contains the pointer
        let mut hover_index = None;
        if let Some(pos) = pointer_pos {
            for (index, square) in self.squares.iter().enumerate() {
                if square.rect.contains(pos) {
                    hover_index = Some(index);
                    break;
                }
            }
        }
        
        // Update the hovered index
        self.hovered_index = hover_index;
        
        // Handle click if needed
        if clicked && hover_index.is_some() {
            let index = hover_index.unwrap();
            
            // Deselect previously selected square if different
            if let Some(prev_index) = self.selected_index {
                if prev_index != index {
                    self.squares[prev_index].selected = false;
                }
            }
            
            // Select the new square
            self.squares[index].selected = true;
            self.selected_index = Some(index);
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
            // Choose color based on type and selection state
            let fill_color = if square.selected {
                egui::Color32::from_rgb(100, 150, 250) // Blue for selected
            } else if square.entry.is_directory {
                egui::Color32::from_rgb(70, 130, 180) // Steel blue for directories
            } else {
                // For files, use the file type color
                self.get_file_color(&square.entry.name)
            };
            
            // Determine corner radius based on type (as u8 for CornerRadius::same)
            let corner_radius = if square.entry.is_directory {
                4 // Rounded corners for directories
            } else {
                2 // Less rounded for files
            };
            
            // Draw the square with appropriate style
            ui.painter().rect_filled(
                square.rect,
                egui::CornerRadius::same(corner_radius),
                fill_color,
            );
            
            // Draw the border with appropriate style
            let border_color = if square.selected {
                egui::Color32::WHITE
            } else {
                egui::Color32::from_rgb(50, 50, 50)
            };
            
            ui.painter().rect_stroke(
                square.rect,
                egui::CornerRadius::same(corner_radius),
                egui::Stroke::new(1.0, border_color),
                egui::epaint::StrokeKind::Middle,
            );
            
            // Draw the name with appropriate style
            let text_color = if square.entry.is_directory {
                egui::Color32::WHITE
            } else {
                // Darker text for files to ensure readability
                egui::Color32::from_rgb(20, 20, 20)
            };
            
            // Adjust text size based on square size
            let font_size = if square.rect.width() > 60.0 {
                14.0
            } else if square.rect.width() > 40.0 {
                12.0
            } else {
                10.0
            };
            
            // Draw name if there's enough space
            if square.rect.width() > 20.0 && square.rect.height() > 20.0 {
                // For files, show a truncated name if needed
                let display_name = if !square.entry.is_directory && square.rect.width() < 80.0 {
                    // Get just the filename without path
                    let filename = square.entry.name.clone();
                    if filename.len() > 10 {
                        format!("{}...", &filename[0..7])
                    } else {
                        filename
                    }
                } else {
                    square.entry.name.clone()
                };
                
                ui.painter().text(
                    square.rect.center(),
                    egui::Align2::CENTER_CENTER,
                    &display_name,
                    egui::FontId::proportional(font_size),
                    text_color,
                );
            }
            
            // For directories at Level2 or MaxIn, show additional info
            if square.entry.is_directory &&
               (self.zoom_level == ZoomLevel::Level2 || self.zoom_level == ZoomLevel::MaxIn) &&
               square.rect.width() > 80.0 {
                // Show item count
                let item_count = square.entry.children.len();
                let info_text = format!("{} items", item_count);
                
                ui.painter().text(
                    square.rect.center_bottom() + egui::vec2(0.0, -10.0),
                    egui::Align2::CENTER_BOTTOM,
                    &info_text,
                    egui::FontId::proportional(10.0),
                    egui::Color32::from_rgba_premultiplied(255, 255, 255, 180),
                );
            }
        }
        
        // Render tooltip if a square is hovered
        if let Some(index) = self.hovered_index {
            let square = &self.squares[index];
            
            // Position tooltip below or to the right of the square depending on space
            let tooltip_pos = if square.rect.right() + 130.0 < canvas_rect.right() {
                square.rect.right_center() + egui::vec2(5.0, 0.0)
            } else {
                square.rect.left_bottom() + egui::vec2(0.0, 5.0)
            };
            
            // Create tooltip content based on entry type
            let tooltip_text = if square.entry.is_directory {
                format!("{}\n{} items", square.entry.name, square.entry.children.len())
            } else {
                // For files, show the full path
                format!("{}", square.entry.name)
            };
            
            // Calculate tooltip size based on content
            let tooltip_width = tooltip_text.len().min(30) as f32 * 7.0;
            let tooltip_height = if tooltip_text.contains('\n') { 40.0 } else { 30.0 };
            
            let tooltip_rect = egui::Rect::from_min_size(
                tooltip_pos,
                egui::vec2(tooltip_width, tooltip_height),
            );
            
            // Draw tooltip background
            ui.painter().rect_filled(
                tooltip_rect,
                egui::CornerRadius::same(2),
                egui::Color32::from_rgb(50, 50, 50),
            );
            
            // Draw tooltip text
            ui.painter().text(
                tooltip_rect.center(),
                egui::Align2::CENTER_CENTER,
                &tooltip_text,
                egui::FontId::proportional(12.0),
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