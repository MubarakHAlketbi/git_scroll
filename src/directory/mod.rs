use std::path::{Path, PathBuf};

/// Represents a directory or file in the repository
#[derive(Debug, Clone)]
pub struct DirectoryEntry {
    /// Name of the directory or file
    pub name: String,
    
    /// Full path to the directory or file
    pub path: PathBuf,
    
    /// Whether this entry is a directory
    pub is_directory: bool,
    
    /// Child entries (empty for files)
    pub children: Vec<DirectoryEntry>,
}

/// Handles parsing and filtering of directory structures
pub struct DirectoryParser {
    /// Patterns to ignore when parsing directories
    ignore_patterns: Vec<String>,
}

impl DirectoryParser {
    /// Collects all files recursively into a flat list
    pub fn get_all_files(&self, entry: &DirectoryEntry) -> Vec<PathBuf> {
        let mut files = Vec::new();
        self.collect_files_recursive(entry, &mut files);
        files
    }

    /// Helper method to recursively collect files
    fn collect_files_recursive(&self, entry: &DirectoryEntry, files: &mut Vec<PathBuf>) {
        if !entry.is_directory {
            if !self.should_ignore(&entry.path) {
                files.push(entry.path.clone());
            }
        } else {
            for child in &entry.children {
                self.collect_files_recursive(child, files);
            }
        }
    }

    /// Creates a new DirectoryParser with default ignore patterns
    ///
    /// # Returns
    /// A new DirectoryParser instance
    pub fn new() -> Self {
        Self {
            // Default patterns to ignore
            ignore_patterns: vec![
                ".git".to_string(),
                "node_modules".to_string(),
                "target".to_string(),
                ".DS_Store".to_string(),
            ],
        }
    }
    
    /// Creates a new DirectoryParser with custom ignore patterns
    /// 
    /// # Arguments
    /// * `ignore_patterns` - Patterns to ignore when parsing directories
    /// 
    /// # Returns
    /// A new DirectoryParser instance with custom ignore patterns
    pub fn with_ignore_patterns(ignore_patterns: Vec<String>) -> Self {
        Self {
            ignore_patterns,
        }
    }
    
    /// Adds an ignore pattern
    /// 
    /// # Arguments
    /// * `pattern` - Pattern to ignore
    pub fn add_ignore_pattern(&mut self, pattern: String) {
        self.ignore_patterns.push(pattern);
    }
    
    /// Parses a directory structure
    ///
    /// # Arguments
    /// * `root_path` - Path to the root directory
    ///
    /// # Returns
    /// Result with the parsed directory structure or an error
    pub fn parse_directory(&self, root_path: &Path) -> Result<DirectoryEntry, String> {
        if !root_path.exists() {
            return Err(format!("Path does not exist: {:?}", root_path));
        }
        
        if !root_path.is_dir() {
            return Err(format!("Path is not a directory: {:?}", root_path));
        }
        
        // Get the directory name
        let root_name = root_path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("root")
            .to_string();
        
        // Recursively parse the directory structure
        self.parse_directory_recursive(root_path, &root_name)
    }
    
    /// Recursively parses a directory structure
    ///
    /// # Arguments
    /// * `dir_path` - Path to the directory
    /// * `dir_name` - Name of the directory
    ///
    /// # Returns
    /// Result with the parsed directory structure or an error
    fn parse_directory_recursive(&self, dir_path: &Path, dir_name: &str) -> Result<DirectoryEntry, String> {
        // Create a vector to store child entries
        let mut children = Vec::new();
        
        // Read the directory entries
        let entries = match std::fs::read_dir(dir_path) {
            Ok(entries) => entries,
            Err(e) => return Err(format!("Failed to read directory: {}", e)),
        };
        
        // Process each entry
        for entry_result in entries {
            // Get the directory entry
            let entry = match entry_result {
                Ok(entry) => entry,
                Err(e) => return Err(format!("Failed to read directory entry: {}", e)),
            };
            
            // Get the path of the entry
            let path = entry.path();
            
            // Skip if the entry should be ignored
            if self.should_ignore(&path) {
                continue;
            }
            
            // Get the metadata of the entry
            let metadata = match entry.metadata() {
                Ok(metadata) => metadata,
                Err(e) => return Err(format!("Failed to get metadata: {}", e)),
            };
            
            // Get the name of the entry
            let name = path.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("unknown")
                .to_string();
            
            // Create a DirectoryEntry for the entry
            if metadata.is_dir() {
                // Recursively parse subdirectories
                match self.parse_directory_recursive(&path, &name) {
                    Ok(child_entry) => children.push(child_entry),
                    Err(e) => return Err(e),
                }
            } else {
                // Add file entry
                children.push(DirectoryEntry {
                    name,
                    path: path.to_path_buf(),
                    is_directory: false,
                    children: Vec::new(),
                });
            }
        }
        
        // Create and return the DirectoryEntry for this directory
        Ok(DirectoryEntry {
            name: dir_name.to_string(),
            path: dir_path.to_path_buf(),
            is_directory: true,
            children,
        })
    }
    
    /// Checks if a path should be ignored
    /// 
    /// # Arguments
    /// * `path` - Path to check
    /// 
    /// # Returns
    /// `true` if the path should be ignored, `false` otherwise
    fn should_ignore(&self, path: &Path) -> bool {
        if let Some(file_name) = path.file_name() {
            if let Some(file_name_str) = file_name.to_str() {
                return self.ignore_patterns.iter().any(|pattern| {
                    file_name_str == pattern || file_name_str.contains(pattern)
                });
            }
        }
        
        false
    }
    
    /// Gets statistics for a directory structure
    ///
    /// # Arguments
    /// * `entry` - The directory entry to analyze
    ///
    /// # Returns
    /// Statistics for the directory structure
    pub fn get_statistics(&self, entry: &DirectoryEntry) -> DirectoryStatistics {
        let mut stats = DirectoryStatistics {
            total_files: 0,
            total_directories: 0,
            total_size_bytes: 0,
            max_depth: 0,
            file_types: HashMap::new(),
        };
        
        // Calculate statistics recursively
        self.calculate_statistics_recursive(entry, &mut stats, 0);
        
        stats
    }
    
    /// Recursively calculates statistics for a directory structure
    ///
    /// # Arguments
    /// * `entry` - The directory entry to analyze
    /// * `stats` - The statistics to update
    /// * `depth` - The current depth in the directory tree
    fn calculate_statistics_recursive(&self, entry: &DirectoryEntry, stats: &mut DirectoryStatistics, depth: usize) {
        // Update max depth
        if depth > stats.max_depth {
            stats.max_depth = depth;
        }
        
        if entry.is_directory {
            // Count this directory
            stats.total_directories += 1;
            
            // Recursively process children
            for child in &entry.children {
                self.calculate_statistics_recursive(child, stats, depth + 1);
            }
        } else {
            // Count this file
            stats.total_files += 1;
            
            // Get file size
            if let Ok(metadata) = std::fs::metadata(&entry.path) {
                stats.total_size_bytes += metadata.len();
            }
            
            // Extract file extension
            if let Some(extension) = entry.path.extension() {
                if let Some(ext_str) = extension.to_str() {
                    let count = stats.file_types.entry(ext_str.to_string())
                        .or_insert(0);
                    *count += 1;
                }
            }
        }
    }
}

/// Statistics for a directory structure
pub struct DirectoryStatistics {
    /// Total number of files
    pub total_files: usize,
    
    /// Total number of directories
    pub total_directories: usize,
    
    /// Total size in bytes
    pub total_size_bytes: u64,
    
    /// Maximum directory depth
    pub max_depth: usize,
    
    /// Count of file types (extension -> count)
    pub file_types: HashMap<String, usize>,
}

// Import HashMap for DirectoryStatistics
use std::collections::HashMap;

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_should_ignore() {
        let parser = DirectoryParser::new();
        
        // Should ignore .git directory
        assert!(parser.should_ignore(Path::new(".git")));
        assert!(parser.should_ignore(Path::new("/path/to/.git")));
        
        // Should ignore node_modules directory
        assert!(parser.should_ignore(Path::new("node_modules")));
        assert!(parser.should_ignore(Path::new("/path/to/node_modules")));
        
        // Should not ignore regular directories
        assert!(!parser.should_ignore(Path::new("src")));
        assert!(!parser.should_ignore(Path::new("/path/to/src")));
    }
    
    #[test]
    fn test_custom_ignore_patterns() {
        let parser = DirectoryParser::with_ignore_patterns(vec![
            "build".to_string(),
            "dist".to_string(),
        ]);
        
        // Should ignore custom patterns
        assert!(parser.should_ignore(Path::new("build")));
        assert!(parser.should_ignore(Path::new("dist")));
        
        // Should not ignore default patterns (they're not included)
        assert!(!parser.should_ignore(Path::new(".git")));
        assert!(!parser.should_ignore(Path::new("node_modules")));
    }
    
    #[test]
    fn test_add_ignore_pattern() {
        let mut parser = DirectoryParser::new();
        
        // Initially should not ignore "temp"
        assert!(!parser.should_ignore(Path::new("temp")));
        
        // Add "temp" to ignore patterns
        parser.add_ignore_pattern("temp".to_string());
        
        // Now should ignore "temp"
        assert!(parser.should_ignore(Path::new("temp")));
    }
}