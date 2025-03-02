use eframe::egui;
use crate::directory::DirectoryEntry;
use crate::directory::DirectoryStatistics;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

/// Represents a visual square in the project area
#[derive(Clone)]
pub struct VisualSquare {
    /// The directory entry this square represents
    pub entry: DirectoryEntry,
    
    /// Position and size of the square
    pub rect: egui::Rect,
    
    /// Whether this square is currently selected
    pub selected: bool,
    
    /// Whether this square is currently hovered
    pub hovered: bool,
    
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

/// Available layout types for visualization
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LayoutType {
    Grid,
    Treemap,
    ForceDirected,
    Detailed,
}

/// Theme options for visualization
#[derive(Debug, Clone, PartialEq)]
pub enum Theme {
    Light,
    Dark,
    Custom(HashMap<String, egui::Color32>),
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
    pub directory_stats: Option<DirectoryStatistics>,
    
    /// Cache of colors for different file types
    file_type_colors: HashMap<String, egui::Color32>,
    
    /// Current layout type
    layout_type: LayoutType,
    
    /// Current theme
    theme: Theme,
    
    /// Dragging index for drag and drop
    dragging_index: Option<usize>,
    
    /// Cache of layout calculations
    layout_cache: HashMap<String, Vec<VisualSquare>>,
    
    /// File content cache for tooltips
    file_content_cache: Arc<Mutex<HashMap<String, String>>>,
    
    /// Flag to indicate if file content is being loaded
    loading_file_content: AtomicBool,
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
            layout_type: LayoutType::Grid,
            theme: Theme::Light,
            dragging_index: None,
            layout_cache: HashMap::new(),
            file_content_cache: Arc::new(Mutex::new(HashMap::new())),
            loading_file_content: AtomicBool::new(false),
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
        
        // Clear layout cache when setting a new root entry
        self.layout_cache.clear();
        
        self.generate_squares();
    }
    
    /// Sets the current layout type
    ///
    /// # Arguments
    /// * `layout_type` - The layout type to use
    pub fn set_layout_type(&mut self, layout_type: LayoutType) {
        if self.layout_type != layout_type {
            self.layout_type = layout_type;
            
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
            
            // Regenerate squares with the new layout
            self.generate_squares();
        }
    }
    
    /// Sets the current theme
    ///
    /// # Arguments
    /// * `theme` - The theme to use
    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
    }
    
    /// Generates visual squares from the directory structure
    fn generate_squares(&mut self) {
        self.squares.clear();
        self.selected_index = None;
        self.hovered_index = None;  // Reset hovered_index to prevent stale indices
        
        if let Some(root) = &self.root_entry {
            // Calculate the available canvas size
            // For now, we'll use a fixed size, but this could be made dynamic
            let canvas_width: f32 = 800.0;
            let canvas_height: f32 = 600.0;
            
            // Clone the root to avoid borrowing issues
            let root_clone = root.clone();
            
            // Check if we have a cached layout for this path and zoom level
            let cache_key = format!("{}_{}_{:?}", 
                root_clone.path.to_string_lossy(), 
                self.zoom_factor,
                self.layout_type
            );
            
            if let Some(cached_squares) = self.layout_cache.get(&cache_key) {
                self.squares = cached_squares.to_vec();
                return;
            }
            
            // Create the layout based on zoom factor and layout type
            match self.layout_type {
                LayoutType::Grid => {
                    let detail_level = ((self.zoom_factor - 1.0) * 10.0) as usize;
                    self.generate_grid_layout(&root_clone, canvas_width, canvas_height, detail_level);
                },
                LayoutType::Treemap => {
                    self.generate_treemap_layout(&root_clone, canvas_width, canvas_height);
                },
                LayoutType::ForceDirected => {
                    self.generate_force_directed_layout(&root_clone, canvas_width, canvas_height);
                },
                LayoutType::Detailed => {
                    self.generate_detailed_layout(&root_clone, canvas_width, canvas_height);
                }
            }
            
            // Cache the generated layout
            self.layout_cache.insert(cache_key, self.squares.clone());
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
        
        // Include both directories and files based on zoom level
        let show_files = self.zoom_factor >= 2.0;
        let entries: Vec<&DirectoryEntry> = if show_files {
            entry.children.iter().collect()
        } else {
            entry.children.iter()
                .filter(|child| child.is_directory)
                .collect()
        };
        
        let entry_count = entries.len();
        if entry_count == 0 {
            return;
        }
        
        // Calculate grid dimensions based on aspect ratio for better layout
        let aspect_ratio = width / height;
        let cols = (entry_count as f32 * aspect_ratio).sqrt().ceil() as usize;
        let rows = (entry_count as f32 / cols as f32).ceil() as usize;
        
        // Calculate cell size
        let cell_width = width / cols as f32;
        let cell_height = height / rows as f32;
        
        // Calculate sizes for variable sizing
        let mut total_size: u64 = 1; // Start with 1 to avoid division by zero
        let mut entry_sizes: Vec<u64> = Vec::with_capacity(entry_count);
        
        for entry in &entries {
            let size = if entry.is_directory {
                // For directories, use the number of children as a size proxy
                // or calculate actual size if needed
                let child_count = entry.children.len();
                (child_count as u64 + 1) * 1000 // Add 1 to avoid zero size
            } else {
                // For files, use the actual file size if available
                if let Ok(metadata) = std::fs::metadata(&entry.path) {
                    let file_size = metadata.len();
                    if file_size > 0 { file_size } else { 1000 } // Minimum size
                } else {
                    1000 // Default size if metadata can't be read
                }
            };
            
            entry_sizes.push(size);
            total_size += size;
        }
        
        // Create squares for each entry with variable sizing
        for (index, (entry, size)) in entries.iter().zip(entry_sizes.iter()).enumerate() {
            let row = index / cols;
            let col = index % cols;
            
            // Base position
            let x = col as f32 * cell_width;
            let y = row as f32 * cell_height;
            
            // Calculate size factor (between 0.5 and 1.5) based on relative size
            let size_factor = 0.5 + ((*size as f32) / (total_size as f32) * entry_count as f32).min(1.0);
            
            // Add padding and adjust size based on size_factor
            let padding = 10.0;
            let adjusted_width = (cell_width - 2.0 * padding) * size_factor;
            let adjusted_height = (cell_height - 2.0 * padding) * size_factor;
            
            // Center the rectangle within its cell
            let x_offset = (cell_width - adjusted_width) / 2.0;
            let y_offset = (cell_height - adjusted_height) / 2.0;
            
            let rect = egui::Rect::from_min_size(
                egui::pos2(x + x_offset, y + y_offset),
                egui::vec2(adjusted_width, adjusted_height),
            );
            
            // Clone the entry to get an owned copy
            let entry_clone = (*entry).clone();
            
            // Calculate size weight for potential future use
            let size_weight = *size as f32 / total_size as f32;
            
            self.squares.push(VisualSquare {
                entry: entry_clone,
                rect,
                selected: false,
                hovered: false,
                size_weight,
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
            let child_size = if let Some(_stats) = &self.directory_stats {
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
    
    /// Generates a force-directed layout for visualizing relationships
    ///
    /// # Arguments
    /// * `entry` - The directory entry to visualize
    /// * `width` - Available width
    /// * `height` - Available height
    fn generate_force_directed_layout(&mut self, entry: &DirectoryEntry, width: f32, height: f32) {
        // Only process if this is a directory
        if !entry.is_directory {
            return;
        }
        
        // Get all children (both directories and files)
        let children = &entry.children;
        
        if children.is_empty() {
            return;
        }
        
        // Create initial positions in a circle
        let center_x = width / 2.0;
        let center_y = height / 2.0;
        let radius = (width.min(height) / 2.0) * 0.8;
        let child_count = children.len();
        
        // Create nodes with initial positions
        for (i, child) in children.iter().enumerate() {
            let angle = 2.0 * std::f32::consts::PI * (i as f32) / (child_count as f32);
            let x = center_x + radius * angle.cos();
            let y = center_y + radius * angle.sin();
            
            // Calculate node size based on file/directory size
            let size = if child.is_directory {
                // Directories are larger
                let child_count = child.children.len();
                (30.0 + (child_count as f32).sqrt() * 5.0).min(80.0)
            } else {
                // Files are smaller
                if let Ok(metadata) = std::fs::metadata(&child.path) {
                    let size_kb = metadata.len() / 1024;
                    (20.0 + (size_kb as f32).sqrt() * 2.0).min(50.0)
                } else {
                    20.0 // Default size
                }
            };
            
            // Clone the entry to get an owned copy
            let child_entry = child.clone();
            
            // Create the rectangle
            let rect = egui::Rect::from_center_size(
                egui::pos2(x, y),
                egui::vec2(size, size),
            );
            
            // Calculate size weight
            let size_weight = if child.is_directory {
                (child.children.len() as f32) / (child_count as f32)
            } else {
                1.0 / (child_count as f32)
            };
            
            // Add the square
            self.squares.push(VisualSquare {
                entry: child_entry,
                rect,
                selected: false,
                hovered: false,
                size_weight,
                animation_progress: 0.0,
                prev_rect: None,
            });
        }
        
        // Apply force-directed algorithm (simplified Fruchterman-Reingold)
        // In a real implementation, this would be iterative with multiple passes
        let iterations = 50;
        let k = (width * height / child_count as f32).sqrt() * 0.3; // Optimal distance
        
        for _ in 0..iterations {
            // Calculate repulsive forces
            for i in 0..self.squares.len() {
                let mut force_x = 0.0;
                let mut force_y = 0.0;
                
                for j in 0..self.squares.len() {
                    if i != j {
                        let dx = self.squares[i].rect.center().x - self.squares[j].rect.center().x;
                        let dy = self.squares[i].rect.center().y - self.squares[j].rect.center().y;
                        let distance = (dx * dx + dy * dy).sqrt().max(0.1);
                        
                        // Repulsive force
                        let force = k * k / distance;
                        force_x += dx / distance * force;
                        force_y += dy / distance * force;
                    }
                }
                
                // Apply forces (with damping)
                let damping = 0.1;
                let new_x = self.squares[i].rect.center().x + force_x * damping;
                let new_y = self.squares[i].rect.center().y + force_y * damping;
                
                // Keep within bounds
                let size = self.squares[i].rect.size();
                let half_width = size.x / 2.0;
                let half_height = size.y / 2.0;
                let bounded_x = new_x.clamp(half_width, width - half_width);
                let bounded_y = new_y.clamp(half_height, height - half_height);
                
                // Update position
                self.squares[i].rect = egui::Rect::from_center_size(
                    egui::pos2(bounded_x, bounded_y),
                    size,
                );
            }
        }
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
                    size_weight: 1.0 / directories.len() as f32, // Equal weight for directories
                    animation_progress: 0.0,
                    prev_rect: None,
                });
            }
        }
        
        // Generate layout for files (if any)
        if !files.is_empty() {
            // Apply Level of Detail (LOD) for files
            let lod_threshold = 0.01; // Threshold for grouping small files
            let mut grouped_files: Vec<DirectoryEntry> = Vec::new();
            let mut _others_size = 0.0;
            let mut others_children = Vec::new();
            
            // Calculate total file size
            let total_file_size: u64 = files.iter()
                .map(|file| {
                    if let Ok(metadata) = std::fs::metadata(&file.path) {
                        metadata.len()
                    } else {
                        1000 // Default size
                    }
                })
                .sum();
            
            // Group small files if zoom factor is low
            if self.zoom_factor < 2.0 && files.len() > 20 {
                for file in &files {
                    let file_size = if let Ok(metadata) = std::fs::metadata(&file.path) {
                        metadata.len()
                    } else {
                        1000 // Default size
                    };
                    
                    let weight = file_size as f32 / total_file_size as f32;
                    
                    if weight < lod_threshold {
                        _others_size += weight;
                        others_children.push((*file).clone());
                    } else {
                        grouped_files.push((*file).clone());
                    }
                }
                
                // Add "Others" group if needed
                if !others_children.is_empty() {
                    // Create a synthetic directory entry for "Others"
                    let others_entry = DirectoryEntry {
                        name: "Others".to_string(),
                        path: entry.path.join("Others"),
                        is_directory: true,
                        children: others_children,
                    };
                    
                    grouped_files.push(others_entry);
                }
            } else {
                grouped_files = files.iter().map(|f| (*f).clone()).collect();
            }
            
            let file_count = grouped_files.len();
            let file_cols = (file_count as f32).sqrt().ceil() as usize;
            let file_rows = (file_count + file_cols - 1) / file_cols;
            
            let file_cell_width = width / file_cols as f32;
            let file_cell_height = file_height / file_rows as f32;
            
            for (index, file) in grouped_files.iter().enumerate() {
                let row = index / file_cols;
                let col = index % file_cols;
                
                let x = col as f32 * file_cell_width;
                let y = dir_height + row as f32 * file_cell_height;
                
                let padding = 5.0;
                let rect = egui::Rect::from_min_size(
                    egui::pos2(x + padding, y + padding),
                    egui::vec2(file_cell_width - 2.0 * padding, file_cell_height - 2.0 * padding),
                );
                
                self.squares.push(VisualSquare {
                    entry: file.clone(),
                    rect,
                    selected: false,
                    hovered: false,
                    size_weight: 1.0 / grouped_files.len() as f32, // Equal weight for files
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
    pub fn handle_interaction(&mut self, ui: &mut egui::Ui, pointer_pos: Option<egui::Pos2>, clicked: bool) {
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
            
            // If it's a directory, set it as the new root entry
            if self.squares[index].entry.is_directory {
                self.set_root_entry(self.squares[index].entry.clone());
                self.animating = true;
                self.animation_start_time = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs_f64();
            }
        }
        
        // Handle right-click for context menu
        if ui.ctx().input(|i| i.pointer.secondary_clicked()) && hover_index.is_some() {
            let index = hover_index.unwrap();
            let pos = ui.ctx().pointer_latest_pos().unwrap_or_default();
            ui.ctx().show_viewport_immediate(
                egui::ViewportId::from_hash_of("context_menu"),
                egui::ViewportBuilder::default()
                    .with_inner_size([200.0, 100.0])
                    .with_position(pos),
                |ctx, _class| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        if self.squares[index].entry.is_directory {
                            if ui.button("Open in Explorer").clicked() {
                                let path = self.squares[index].entry.path.clone();
                                std::process::Command::new("explorer")
                                    .arg(path)
                                    .spawn()
                                    .ok();
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            }
                            if ui.button("Set as Root").clicked() {
                                self.set_root_entry(self.squares[index].entry.clone());
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            }
                        } else {
                            if ui.button("View Content").clicked() {
                                self.load_file_content(&self.squares[index].entry.path);
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            }
                            if ui.button("Open in Default App").clicked() {
                                let path = self.squares[index].entry.path.clone();
                                std::process::Command::new("cmd")
                                    .args(&["/c", "start", "", path.to_string_lossy().as_ref()])
                                    .spawn()
                                    .ok();
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            }
                        }
                    });
                },
            );
        }
        
        // Handle drag and drop
        if let Some(pos) = pointer_pos {
            if ui.input(|i| i.pointer.primary_down()) {
                if self.dragging_index.is_none() && hover_index.is_some() {
                    self.dragging_index = hover_index;
                }
            } else if self.dragging_index.is_some() && !ui.input(|i| i.pointer.primary_down()) {
                self.dragging_index = None;
            } else if let Some(index) = self.dragging_index {
                // Update position of dragged square
                let size = self.squares[index].rect.size();
                self.squares[index].rect = egui::Rect::from_center_size(
                    pos,
                    size,
                );
            }
        }
    }
    
    /// Loads file content asynchronously for tooltips
    ///
    /// # Arguments
    /// * `path` - Path to the file
    fn load_file_content(&self, path: &std::path::Path) {
        let path_str = path.to_string_lossy().to_string();
        
        // Check if we already have the content cached
        if let Ok(cache) = self.file_content_cache.lock() {
            if cache.contains_key(&path_str) {
                return;
            }
        }
        
        // Set loading flag using atomic operation
        self.loading_file_content.store(true, Ordering::SeqCst);
        
        // Clone the path and cache for the thread
        let path_clone = path.to_path_buf();
        let cache_clone = Arc::clone(&self.file_content_cache);
        
        // Spawn a thread to load the file content
        thread::spawn(move || {
            let content = std::fs::read_to_string(&path_clone)
                .map(|s| s.lines().take(5).collect::<Vec<_>>().join("\n"))
                .unwrap_or_else(|_| "Content unavailable".to_string());
            
            // Store in cache
            if let Ok(mut cache) = cache_clone.lock() {
                cache.insert(path_str, content);
            }
        });
    }
    
    /// Renders the visualization
    ///
    /// # Arguments
    /// * `ui` - The egui UI to render to
    pub fn render(&self, ui: &mut egui::Ui) {
        let canvas_rect = ui.available_rect_before_wrap();
        
        // Draw the canvas background
        let bg_color = match self.theme {
            Theme::Light => egui::Color32::from_rgb(240, 240, 240),
            Theme::Dark => egui::Color32::from_rgb(30, 30, 30),
            Theme::Custom(ref colors) => *colors.get("background").unwrap_or(&egui::Color32::from_rgb(240, 240, 240)),
        };
        
        ui.painter().rect_filled(
            canvas_rect,
            0.0,
            bg_color,
        );
        
        // Draw each square
        for square in &self.squares {
            // Calculate opacity based on zoom factor for semantic transitions
            let file_opacity = if !square.entry.is_directory {
                // Files fade in from zoom factor 1.0 to 2.0
                ((self.zoom_factor - 1.0) / 1.0).clamp(0.0, 1.0)
            } else {
                1.0 // Directories are always fully visible
            };
            
            // Choose color based on type, selection state, and opacity
            let fill_color = if square.selected {
                match self.theme {
                    Theme::Light => egui::Color32::from_rgb(100, 150, 250), // Blue for selected
                    Theme::Dark => egui::Color32::from_rgb(80, 120, 200),   // Darker blue for dark theme
                    Theme::Custom(ref colors) => *colors.get("selected").unwrap_or(&egui::Color32::from_rgb(100, 150, 250)),
                }
            } else if square.entry.is_directory {
                match self.theme {
                    Theme::Light => egui::Color32::from_rgb(70, 130, 180), // Steel blue for directories
                    Theme::Dark => egui::Color32::from_rgb(60, 100, 140),  // Darker blue for dark theme
                    Theme::Custom(ref colors) => *colors.get("directory").unwrap_or(&egui::Color32::from_rgb(70, 130, 180)),
                }
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
            } else if square.hovered {
                // Add glow effect for hovered squares
                egui::Color32::from_rgb(255, 255, 0) // Yellow glow
            } else {
                match self.theme {
                    Theme::Light => egui::Color32::from_rgb(50, 50, 50),
                    Theme::Dark => egui::Color32::from_rgb(150, 150, 150),
                    Theme::Custom(ref colors) => *colors.get("border").unwrap_or(&egui::Color32::from_rgb(50, 50, 50)),
                }
            };
            
            // Draw border with appropriate stroke
            let stroke_width = if square.hovered { 2.0 } else { 1.0 };
            ui.painter().rect_stroke(
                draw_rect,
                egui::CornerRadius::same(corner_radius),
                egui::Stroke::new(stroke_width, border_color),
                egui::epaint::StrokeKind::Middle,
            );
            
            // Draw the name with appropriate style
            let text_color = if square.entry.is_directory {
                match self.theme {
                    Theme::Light => egui::Color32::WHITE,
                    Theme::Dark => egui::Color32::WHITE,
                    Theme::Custom(ref colors) => *colors.get("directory_text").unwrap_or(&egui::Color32::WHITE),
                }
            } else {
                // Darker text for files to ensure readability
                match self.theme {
                    Theme::Light => egui::Color32::from_rgb(20, 20, 20),
                    Theme::Dark => egui::Color32::from_rgb(220, 220, 220),
                    Theme::Custom(ref colors) => *colors.get("file_text").unwrap_or(&egui::Color32::from_rgb(20, 20, 20)),
                }
            };
            
            // Adjust text size based on square size (using the animated rectangle)
            let font_size = if draw_rect.width() > 60.0 {
                14.0
            } else if draw_rect.width() > 40.0 {
                12.0
            } else {
                10.0
            };
            
            // Draw name and size information if there's enough space
            if draw_rect.width() > 20.0 && draw_rect.height() > 20.0 {
                // Prepare the display text with name and size information
                let name = &square.entry.name;
                let size_info = if square.entry.is_directory {
                    format!("{} items", square.entry.children.len())
                } else {
                    if let Ok(metadata) = std::fs::metadata(&square.entry.path) {
                        format!("{} KB", metadata.len() / 1024)
                    } else {
                        "Unknown size".to_string()
                    }
                };
                
                // For smaller squares, show just the name with truncation if needed
                let display_text = if draw_rect.width() < 80.0 {
                    if name.len() > 10 {
                        format!("{}...", &name[0..7])
                    } else {
                        name.clone()
                    }
                } else {
                    // For larger squares, show name and size information
                    format!("{}\n{}", name, size_info)
                };
                
                ui.painter().text(
                    draw_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    &display_text,
                    egui::FontId::proportional(font_size),
                    text_color,
                );
            }
            
            // For directories at higher zoom levels, show additional info
            let details_opacity = ((self.zoom_factor - 2.0) / 1.0).clamp(0.0, 1.0); // Fades in from 2.0 to 3.0
            if square.entry.is_directory && details_opacity > 0.0 && draw_rect.width() > 80.0 {
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
            let mut tooltip_text = if square.entry.is_directory {
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
            
            // Add file content preview for files at high zoom levels
            if content_detail > 0.5 && !square.entry.is_directory {
                let path_str = square.entry.path.to_string_lossy().to_string();
                
                // Check if we have the content cached
                if let Ok(cache) = self.file_content_cache.lock() {
                    if let Some(content) = cache.get(&path_str) {
                        tooltip_text = format!("{}\n\n{}", tooltip_text, content);
                    }
                }
            }
            
            // Calculate tooltip size based on content
            let tooltip_width = tooltip_text.len().min(50) as f32 * 7.0;
            let line_count = tooltip_text.matches('\n').count() + 1;
            let tooltip_height = line_count as f32 * 20.0;
            
            let tooltip_rect = egui::Rect::from_min_size(
                tooltip_pos,
                egui::vec2(tooltip_width, tooltip_height),
            );
            
            // Draw tooltip background
            let tooltip_bg_color = match self.theme {
                Theme::Light => egui::Color32::from_rgb(50, 50, 50),
                Theme::Dark => egui::Color32::from_rgb(70, 70, 70),
                Theme::Custom(ref colors) => *colors.get("tooltip_bg").unwrap_or(&egui::Color32::from_rgb(50, 50, 50)),
            };
            
            ui.painter().rect_filled(
                tooltip_rect,
                egui::CornerRadius::same(2),
                tooltip_bg_color,
            );
            
            // Draw tooltip text
            let tooltip_text_color = match self.theme {
                Theme::Light => egui::Color32::WHITE,
                Theme::Dark => egui::Color32::WHITE,
                Theme::Custom(ref colors) => *colors.get("tooltip_text").unwrap_or(&egui::Color32::WHITE),
            };
            
            ui.painter().text(
                tooltip_rect.center(),
                egui::Align2::CENTER_CENTER,
                &tooltip_text,
                egui::FontId::proportional(12.0),
                tooltip_text_color,
            );
        }
    }
    
    /// Initializes the color map for different file types
    /// 
    /// # Returns
    /// A HashMap mapping file extensions to colors
    fn initialize_file_colors() -> HashMap<String, egui::Color32> {
        let mut colors = HashMap::new();
        
        // Add colors for common file types using a perceptually uniform palette
        // Code files
        colors.insert("rs".to_string(), egui::Color32::from_rgb(250, 100, 100)); // Rust files - Red
        colors.insert("js".to_string(), egui::Color32::from_rgb(240, 220, 100)); // JavaScript - Yellow
        colors.insert("py".to_string(), egui::Color32::from_rgb(100, 200, 150)); // Python - Green
        colors.insert("java".to_string(), egui::Color32::from_rgb(180, 120, 80)); // Java - Brown
        colors.insert("c".to_string(), egui::Color32::from_rgb(100, 160, 200)); // C - Blue
        colors.insert("cpp".to_string(), egui::Color32::from_rgb(120, 140, 220)); // C++ - Purple-blue
        colors.insert("h".to_string(), egui::Color32::from_rgb(140, 180, 220)); // Header - Light blue
        colors.insert("cs".to_string(), egui::Color32::from_rgb(100, 180, 180)); // C# - Teal
        
        // Web files
        colors.insert("html".to_string(), egui::Color32::from_rgb(255, 159, 64)); // HTML - Orange
        colors.insert("css".to_string(), egui::Color32::from_rgb(153, 102, 255)); // CSS - Purple
        colors.insert("scss".to_string(), egui::Color32::from_rgb(173, 122, 255)); // SCSS - Light purple
        colors.insert("less".to_string(), egui::Color32::from_rgb(133, 102, 235)); // LESS - Dark purple
        colors.insert("svg".to_string(), egui::Color32::from_rgb(255, 120, 180)); // SVG - Pink
        
        // Data files
        colors.insert("json".to_string(), egui::Color32::from_rgb(250, 150, 100)); // JSON - Orange
        colors.insert("xml".to_string(), egui::Color32::from_rgb(200, 150, 200)); // XML - Light purple
        colors.insert("yaml".to_string(), egui::Color32::from_rgb(180, 200, 120)); // YAML - Light green
        colors.insert("yml".to_string(), egui::Color32::from_rgb(180, 200, 120)); // YML - Light green
        colors.insert("toml".to_string(), egui::Color32::from_rgb(200, 180, 140)); // TOML - Tan
        colors.insert("csv".to_string(), egui::Color32::from_rgb(160, 220, 160)); // CSV - Light green
        
        // Document files
        colors.insert("md".to_string(), egui::Color32::from_rgb(150, 150, 250)); // Markdown - Blue
        colors.insert("txt".to_string(), egui::Color32::from_rgb(200, 200, 200)); // Text - Gray
        colors.insert("pdf".to_string(), egui::Color32::from_rgb(240, 100, 100)); // PDF - Red
        colors.insert("doc".to_string(), egui::Color32::from_rgb(100, 150, 250)); // DOC - Blue
        colors.insert("docx".to_string(), egui::Color32::from_rgb(100, 150, 250)); // DOCX - Blue
        
        // Image files
        colors.insert("png".to_string(), egui::Color32::from_rgb(100, 220, 200)); // PNG - Teal
        colors.insert("jpg".to_string(), egui::Color32::from_rgb(120, 200, 220)); // JPG - Light blue
        colors.insert("jpeg".to_string(), egui::Color32::from_rgb(120, 200, 220)); // JPEG - Light blue
        colors.insert("gif".to_string(), egui::Color32::from_rgb(200, 120, 220)); // GIF - Pink
        colors.insert("webp".to_string(), egui::Color32::from_rgb(150, 220, 200)); // WEBP - Light teal
        
        // Config files
        colors.insert("gitignore".to_string(), egui::Color32::from_rgb(150, 150, 150)); // Gitignore - Gray
        colors.insert("env".to_string(), egui::Color32::from_rgb(120, 180, 120)); // Env - Green
        colors.insert("lock".to_string(), egui::Color32::from_rgb(180, 180, 180)); // Lock - Gray
        
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
        let zoom_step: f32 = 0.2; // Slightly larger increments for more noticeable zooming
        let new_target = if zoom_in {
            (self.zoom_factor + zoom_step).min(4.0) // Max zoom is 4.0
        } else {
            (self.zoom_factor - zoom_step).max(1.0) // Min zoom is 1.0
        };
        
        // If no significant change, return early
        if (new_target - self.target_zoom_factor).abs() < 0.01 {
            return;
        }
        
        // Clear layout cache to force regeneration with new zoom level
        if let Some(root) = &self.root_entry {
            let cache_key = format!("{}_{}_{:?}",
                root.path.to_string_lossy(),
                new_target,  // Use new_target instead of current zoom_factor
                self.layout_type
            );
            self.layout_cache.remove(&cache_key);
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
            // Use generate_squares instead of set_root_entry to avoid clearing the cache
            self.generate_squares();
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