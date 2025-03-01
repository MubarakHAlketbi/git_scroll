use eframe::egui;
use crate::directory::DirectoryEntry;
use crate::directory::DirectoryStatistics;
use std::collections::HashMap;

/// Represents a visual square in the project area
pub struct VisualSquare {
    /// The directory entry this square represents
    pub entry: DirectoryEntry,
    
    /// Position and size of the square
    pub rect: egui::Rect,
    
    /// Whether this square is currently selected
    pub selected: bool,
    
    /// Whether this square is currently hovered
    pub hovered: bool,
    
    /// Whether this square represents a directory (vs a file)
    pub is_directory: bool,
    
    /// Size weight for proportional sizing (based on file/directory size)
    pub size_weight: f32,
    
    /// Animation progress for transitions (0.0 to 1.0)
    pub animation_progress: f32,
    
    /// Previous rectangle (for animation)
    pub prev_rect: Option<egui::Rect>,
}

/// Helper function for smooth animation with ease-in-out quadratic interpolation
///
/// # Arguments
/// * `t` - Progress value between 0.0 and 1.0
///
/// # Returns
/// Eased value between 0.0 and 1.0
fn ease_in_out_quad(t: f32) -> f32 {
    if t < 0.5 {
        2.0 * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
    }
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
    
    /// Current zoom factor (1.0 to 4.0)
    zoom_factor: f32,
    
    /// Target zoom factor during transitions
    target_zoom_factor: f32,
    
    /// Animation in progress flag
    animating: bool,
    
    /// Animation start time
    animation_start_time: f64,
    
    /// Animation duration in seconds
    animation_duration: f64,
    
    /// Directory statistics for size-based visualization
    directory_stats: Option<DirectoryStatistics>,
    
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
            zoom_factor: 1.0, // Start at minimum zoom (equivalent to MaxOut)
            target_zoom_factor: 1.0,
            animating: false,
            animation_start_time: 0.0,
            animation_duration: 0.3, // 300ms animation duration
            directory_stats: None,
            file_type_colors: Self::initialize_file_colors(),
        }
    }
    
    /// Sets the root directory entry to visualize
    ///
    /// # Arguments
    /// * `entry` - The root directory entry
    pub fn set_root_entry(&mut self, entry: DirectoryEntry) {
        self.root_entry = Some(entry.clone());
        
        // Calculate directory statistics for size-based visualization
        let parser = crate::directory::DirectoryParser::new();
        self.directory_stats = Some(parser.get_statistics(&entry));
        
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
            
            // Create the layout based on zoom factor
            if self.zoom_factor < 2.0 {
                // At lower zoom (1.0-2.0), use grid layout with increasing detail
                let detail_level = ((self.zoom_factor - 1.0) * 10.0) as usize;
                self.generate_grid_layout(&root_clone, canvas_width, canvas_height, detail_level);
            } else if self.zoom_factor < 3.0 {
                // At medium zoom (2.0-3.0), use treemap layout
                self.generate_treemap_layout(&root_clone, canvas_width, canvas_height);
            } else {
                // At higher zoom (3.0-4.0), show detailed layout
                self.generate_detailed_layout(&root_clone, canvas_width, canvas_height);
            }
        }
    }
    
    /// Generates a grid layout for directories
    ///
    /// # Arguments
    /// * `entry` - The directory entry to visualize
    /// * `width` - Available width
    /// * `height` - Available height
    /// * `_depth` - Current depth in the directory tree (unused but kept for future use)
    fn generate_grid_layout(&mut self, entry: &DirectoryEntry, width: f32, height: f32, _depth: usize) {
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
                hovered: false,
                is_directory: true,
                size_weight: 1.0 / dir_count as f32, // Equal weight for grid layout
                animation_progress: 0.0,
                prev_rect: None,
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
        
        // Calculate total size based on actual file/directory sizes if available
        let mut total_size: u64 = 0;
        let mut child_sizes: HashMap<String, u64> = HashMap::new();
        
        // First pass: calculate sizes
        for child in children {
            let child_size = if let Some(stats) = &self.directory_stats {
                if child.is_directory {
                    // For directories, use the total size of all contained files
                    // Get metadata for this directory
                    if let Ok(metadata) = std::fs::metadata(&child.path) {
                        // Use directory size or fallback to child count * 1000 as a proxy
                        if metadata.len() > 0 {
                            metadata.len()
                        } else {
                            (child.children.len() as u64) * 1000
                        }
                    } else {
                        (child.children.len() as u64) * 1000
                    }
                } else {
                    // For files, use the actual file size
                    if let Ok(metadata) = std::fs::metadata(&child.path) {
                        metadata.len()
                    } else {
                        1000 // Default size if metadata can't be read
                    }
                }
            } else {
                // Fallback if no stats available
                if child.is_directory {
                    (child.children.len() as u64) * 1000
                } else {
                    1000
                }
            };
            
            // Store the size for this child
            child_sizes.insert(child.path.to_string_lossy().to_string(), child_size);
            total_size += child_size;
        }
        
        // Ensure total_size is not zero to avoid division by zero
        if total_size == 0 {
            total_size = 1;
        }
        
        // Sort children by size (larger items first)
        let mut sorted_children: Vec<&DirectoryEntry> = children.iter().collect();
        sorted_children.sort_by(|a, b| {
            let a_size = child_sizes.get(&a.path.to_string_lossy().to_string()).unwrap_or(&0);
            let b_size = child_sizes.get(&b.path.to_string_lossy().to_string()).unwrap_or(&0);
            b_size.cmp(a_size)
        });
        
        // Use a squarified treemap algorithm for better aspect ratios
        self.generate_squarified_treemap(
            sorted_children,
            child_sizes,
            total_size,
            egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(width, height)
            )
        );
    }
    
    /// Generates a squarified treemap layout for better aspect ratios
    ///
    /// # Arguments
    /// * `children` - Sorted children (largest first)
    /// * `child_sizes` - Map of child path to size
    /// * `total_size` - Total size of all children
    /// * `rect` - Available rectangle
    fn generate_squarified_treemap(
        &mut self,
        children: Vec<&DirectoryEntry>,
        child_sizes: HashMap<String, u64>,
        total_size: u64,
        rect: egui::Rect
    ) {
        if children.is_empty() {
            return;
        }
        
        let padding = 5.0;
        let available_rect = egui::Rect::from_min_size(
            rect.min + egui::vec2(padding, padding),
            rect.size() - egui::vec2(padding * 2.0, padding * 2.0)
        );
        
        // Determine if we're laying out horizontally or vertically
        let is_horizontal = available_rect.width() >= available_rect.height();
        
        let mut current_pos = available_rect.min;
        let mut remaining_rect = available_rect;
        
        for child in children {
            // Get the size for this child
            let child_size = child_sizes.get(&child.path.to_string_lossy().to_string()).unwrap_or(&0);
            
            // Calculate the proportion of the total size
            let size_proportion = *child_size as f32 / total_size as f32;
            
            // Calculate the area for this item
            let item_area = available_rect.width() * available_rect.height() * size_proportion;
            
            // Calculate dimensions based on orientation
            let (item_width, item_height) = if is_horizontal {
                let item_width = item_area / remaining_rect.height();
                (item_width, remaining_rect.height())
            } else {
                let item_height = item_area / remaining_rect.width();
                (remaining_rect.width(), item_height)
            };
            
            // Create the rectangle
            let item_rect = egui::Rect::from_min_size(
                current_pos,
                egui::vec2(item_width, item_height)
            );
            
            // Clone the directory entry to get an owned copy
            let child_entry = (*child).clone();
            
            // Calculate size weight for potential future use
            let size_weight = size_proportion;
            
            // Add the square
            self.squares.push(VisualSquare {
                entry: child_entry,
                rect: item_rect,
                selected: false,
                hovered: false,
                is_directory: child_entry.is_directory,
                size_weight,
                animation_progress: 0.0,
                prev_rect: None,
            });
            
            // Update position and remaining rectangle for next item
            if is_horizontal {
                current_pos.x += item_width;
                remaining_rect = egui::Rect::from_min_size(
                    egui::pos2(current_pos.x, remaining_rect.min.y),
                    egui::vec2(remaining_rect.width() - item_width, remaining_rect.height())
                );
            } else {
                current_pos.y += item_height;
                remaining_rect = egui::Rect::from_min_size(
                    egui::pos2(remaining_rect.min.x, current_pos.y),
                    egui::vec2(remaining_rect.width(), remaining_rect.height() - item_height)
                );
            }
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
                    hovered: false,
                    is_directory: true,
                    size_weight: 1.0 / directories.len() as f32, // Equal weight for directories
                    animation_progress: 0.0,
                    prev_rect: None,
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
                    hovered: false,
                    is_directory: false,
                    size_weight: 1.0 / files.len() as f32, // Equal weight for files
                    animation_progress: 0.0,
                    prev_rect: None,
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
            // Calculate opacity based on zoom factor for semantic transitions
            let file_opacity = if !square.is_directory {
                // Files fade in from zoom factor 1.0 to 2.0
                ((self.zoom_factor - 1.0) / 1.0).clamp(0.0, 1.0)
            } else {
                1.0 // Directories are always fully visible
            };
            
            // Choose color based on type, selection state, and opacity
            let fill_color = if square.selected {
                egui::Color32::from_rgb(100, 150, 250) // Blue for selected
            } else if square.is_directory {
                egui::Color32::from_rgb(70, 130, 180) // Steel blue for directories
            } else {
                // For files, use the file type color with calculated opacity
                let base_color = self.get_file_color(&square.entry.name);
                let alpha = (file_opacity * 255.0) as u8;
                egui::Color32::from_rgba_unmultiplied(base_color.r(), base_color.g(), base_color.b(), alpha)
            };
            
            // Determine corner radius based on type (as u8 for CornerRadius::same)
            let corner_radius = if square.entry.is_directory {
                4 // Rounded corners for directories
            } else {
                2 // Less rounded for files
            };
            
            // Calculate the actual rectangle to draw based on animation
            let draw_rect = if self.animating && square.prev_rect.is_some() {
                // Interpolate between previous and current rectangle
                let prev_rect = square.prev_rect.unwrap();
                let progress = square.animation_progress;
                
                // Linear interpolation between rectangles
                let min_x = prev_rect.min.x + (square.rect.min.x - prev_rect.min.x) * progress;
                let min_y = prev_rect.min.y + (square.rect.min.y - prev_rect.min.y) * progress;
                let max_x = prev_rect.max.x + (square.rect.max.x - prev_rect.max.x) * progress;
                let max_y = prev_rect.max.y + (square.rect.max.y - prev_rect.max.y) * progress;
                
                egui::Rect::from_min_max(
                    egui::pos2(min_x, min_y),
                    egui::pos2(max_x, max_y)
                )
            } else {
                square.rect
            };
            
            // Draw the square with appropriate style
            ui.painter().rect_filled(
                draw_rect,
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
                draw_rect,
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
            
            // Adjust text size based on square size (using the animated rectangle)
            let font_size = if draw_rect.width() > 60.0 {
                14.0
            } else if draw_rect.width() > 40.0 {
                12.0
            } else {
                10.0
            };
            
            // Draw name if there's enough space
            if draw_rect.width() > 20.0 && draw_rect.height() > 20.0 {
                // For files, show a truncated name if needed
                let display_name = if !square.entry.is_directory && draw_rect.width() < 80.0 {
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
                    draw_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    &display_name,
                    egui::FontId::proportional(font_size),
                    text_color,
                );
            }
            
            // For directories at higher zoom levels, show additional info
            let details_opacity = ((self.zoom_factor - 2.0) / 1.0).clamp(0.0, 1.0); // Fades in from 2.0 to 3.0
            if square.is_directory && details_opacity > 0.0 && draw_rect.width() > 80.0 {
                // Show item count
                let item_count = square.entry.children.len();
                let info_text = format!("{} items", item_count);
                
                // Apply opacity based on zoom level for smooth fade-in
                let text_alpha = (details_opacity * 180.0) as u8;
                ui.painter().text(
                    draw_rect.center_bottom() + egui::vec2(0.0, -10.0),
                    egui::Align2::CENTER_BOTTOM,
                    &info_text,
                    egui::FontId::proportional(10.0),
                    egui::Color32::from_rgba_premultiplied(255, 255, 255, text_alpha),
                );
            }
        }
        
        // Render tooltip if a square is hovered
        if let Some(index) = self.hovered_index {
            let square = &self.squares[index];
            
            // Get the actual rectangle to use for tooltip positioning
            let rect_for_tooltip = if self.animating && square.prev_rect.is_some() {
                // Use the interpolated rectangle for tooltip positioning
                let prev_rect = square.prev_rect.unwrap();
                let progress = square.animation_progress;
                
                // Linear interpolation between rectangles
                let min_x = prev_rect.min.x + (square.rect.min.x - prev_rect.min.x) * progress;
                let min_y = prev_rect.min.y + (square.rect.min.y - prev_rect.min.y) * progress;
                let max_x = prev_rect.max.x + (square.rect.max.x - prev_rect.max.x) * progress;
                let max_y = prev_rect.max.y + (square.rect.max.y - prev_rect.max.y) * progress;
                
                egui::Rect::from_min_max(
                    egui::pos2(min_x, min_y),
                    egui::pos2(max_x, max_y)
                )
            } else {
                square.rect
            };
            
            // Position tooltip below or to the right of the square depending on space
            let tooltip_pos = if rect_for_tooltip.right() + 130.0 < canvas_rect.right() {
                rect_for_tooltip.right_center() + egui::vec2(5.0, 0.0)
            } else {
                rect_for_tooltip.left_bottom() + egui::vec2(0.0, 5.0)
            };
            
            // Calculate content detail level based on zoom factor
            let content_detail = ((self.zoom_factor - 3.0) / 1.0).clamp(0.0, 1.0); // Fades in from 3.0 to 4.0
            
            // Create tooltip content based on entry type and zoom level
            let tooltip_text = if square.is_directory {
                if content_detail > 0.5 && !square.entry.children.is_empty() {
                    // At high zoom, show more details about directory contents
                    let child_count = square.entry.children.len();
                    let file_count = square.entry.children.iter().filter(|c| !c.is_directory).count();
                    let dir_count = child_count - file_count;
                    format!("{}\n{} items ({} files, {} dirs)",
                            square.entry.name, child_count, file_count, dir_count)
                } else {
                    format!("{}\n{} items", square.entry.name, square.entry.children.len())
                }
            } else {
                // For files, show more details at higher zoom levels
                if content_detail > 0.5 {
                    // At high zoom, show file details like size if available
                    if let Ok(metadata) = std::fs::metadata(&square.entry.path) {
                        let size_kb = metadata.len() / 1024;
                        format!("{}\nSize: {} KB", square.entry.name, size_kb)
                    } else {
                        format!("{}", square.entry.name)
                    }
                } else {
                    format!("{}", square.entry.name)
                }
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
    
    /// Zooms the visualization in or out with animation
    ///
    /// # Arguments
    /// * `zoom_in` - Whether to zoom in (true) or out (false)
    pub fn zoom(&mut self, zoom_in: bool) {
        // Calculate the new target zoom factor with fine-grained control
        let zoom_step: f32 = 0.1; // Small increments for smooth zooming
        let new_target = if zoom_in {
            (self.zoom_factor + zoom_step).min(4.0) // Max zoom is 4.0
        } else {
            (self.zoom_factor - zoom_step).max(1.0) // Min zoom is 1.0
        };
        
        // If no significant change, return early
        if (new_target - self.target_zoom_factor).abs() < 0.01 {
            return;
        }
        
        self.target_zoom_factor = new_target;
        
        // Store the current state for animation
        for square in &mut self.squares {
            square.prev_rect = Some(square.rect);
            square.animation_progress = 0.0;
        }
        
        // Start animation
        self.animating = true;
        self.animation_start_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();
        
        // Regenerate the visualization based on the new zoom factor
        if let Some(root) = &self.root_entry {
            self.set_root_entry(root.clone());
        }
    }
    
    /// Updates animation state
    ///
    /// # Arguments
    /// * `now` - Current time in seconds
    pub fn update_animation(&mut self, now: f64) {
        if !self.animating {
            return;
        }
        
        // Calculate elapsed time
        let elapsed = now - self.animation_start_time;
        
        // Calculate progress (0.0 to 1.0)
        let progress = (elapsed / self.animation_duration).min(1.0);
        
        // Apply easing function for smoother animation
        let eased_progress = ease_in_out_quad(progress as f32);
        
        // Interpolate zoom factor
        self.zoom_factor = self.zoom_factor + (self.target_zoom_factor - self.zoom_factor) * eased_progress;
        
        // Update animation progress for all squares
        for square in &mut self.squares {
            square.animation_progress = eased_progress;
        }
        
        // Check if animation is complete
        if progress >= 1.0 {
            self.animating = false;
            self.zoom_factor = self.target_zoom_factor; // Ensure we reach exactly the target
            
            // Clear previous rectangles
            for square in &mut self.squares {
                square.prev_rect = None;
            }
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
    fn test_zoom_factors() {
        let mut visualizer = Visualizer::new();
        
        // Start at minimum zoom (1.0)
        assert_eq!(visualizer.zoom_factor, 1.0);
        
        // Zoom in once
        visualizer.zoom(true);
        assert!(visualizer.target_zoom_factor > 1.0);
        
        // Zoom in several times to reach maximum
        for _ in 0..30 {
            visualizer.zoom(true);
        }
        
        // Should be capped at maximum (4.0)
        assert_eq!(visualizer.target_zoom_factor, 4.0);
        
        // Zoom out once
        visualizer.zoom(false);
        assert!(visualizer.target_zoom_factor < 4.0);
        
        // Zoom out several times to reach minimum
        for _ in 0..30 {
            visualizer.zoom(false);
        }
        
        // Should be capped at minimum (1.0)
        assert_eq!(visualizer.target_zoom_factor, 1.0);
    }
    
    #[test]
    fn test_easing_function() {
        // Test start, middle and end points
        assert_eq!(ease_in_out_quad(0.0), 0.0);
        assert!(ease_in_out_quad(0.25) < 0.25); // Slower at the beginning
        assert_eq!(ease_in_out_quad(0.5), 0.5); // Linear at the middle
        assert!(ease_in_out_quad(0.75) > 0.75); // Faster approaching the end
        assert_eq!(ease_in_out_quad(1.0), 1.0);
    }
}