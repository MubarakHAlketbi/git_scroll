use std::path::{Path, PathBuf};
use regex::Regex;

/// Handles Git repository operations
pub struct GitHandler {
    /// Whether to keep the repository after cloning
    keep_repository: bool,
}

impl GitHandler {
    /// Creates a new GitHandler
    /// 
    /// # Arguments
    /// * `keep_repository` - Whether to keep the repository after cloning
    /// 
    /// # Returns
    /// A new GitHandler instance
    pub fn new(keep_repository: bool) -> Self {
        Self {
            keep_repository,
        }
    }
    
    /// Validates a Git URL format
    ///
    /// # Arguments
    /// * `url` - The URL to validate
    ///
    /// # Returns
    /// `true` if the URL is valid, `false` otherwise
    pub fn validate_url(url: &str) -> bool {
        // Enhanced validation for Git URLs supporting HTTPS, SSH, and local paths
        // with optional .git suffix
        let re = Regex::new(r"^(https://|git@|file://|/).*(\.git)?$").unwrap();
        re.is_match(url)
    }
    
    /// Clones a Git repository with improved error handling
    ///
    /// # Arguments
    /// * `url` - The Git URL to clone
    /// * `destination` - The destination path
    ///
    /// # Returns
    /// Result with the path to the cloned repository or a detailed error message
    pub fn clone_repository(&self, url: &str, destination: &Path) -> Result<PathBuf, String> {
        // First, ensure the destination is valid
        if destination.exists() && !destination.is_dir() {
            return Err(format!("Destination path exists but is not a directory: {}",
                destination.display()));
        }
        
        // Attempt to clone the repository
        let repo = match git2::Repository::clone(url, destination) {
            Ok(repo) => repo,
            Err(e) => {
                // If cloning fails and the URL doesn't end with .git, try appending .git
                if !url.ends_with(".git") {
                    let url_with_git = format!("{}.git", url);
                    match git2::Repository::clone(&url_with_git, destination) {
                        Ok(repo) => repo,
                        Err(e2) => {
                            // Provide detailed error information for both attempts
                            return Err(format!(
                                "Failed to clone repository:\n- Original URL ({}): {}\n- With .git suffix ({}): {}",
                                url, e, url_with_git, e2
                            ));
                        }
                    }
                } else {
                    // Categorize common errors for better user feedback
                    let error_msg = match e.code() {
                        git2::ErrorCode::Auth => format!(
                            "Authentication failed for {}. Check your credentials or ensure the repository is public.",
                            url
                        ),
                        git2::ErrorCode::NotFound => format!(
                            "Repository not found: {}. Verify the URL is correct.",
                            url
                        ),
                        _ => format!(
                            "Failed to clone {}. Error: {}. Check your internet connection and URL.",
                            url, e
                        ),
                    };
                    return Err(error_msg);
                }
            }
        };
        
        // Return the path to the repository
        // We use path() to get the .git directory and parent_path() to get the repository root
        // We use to_path_buf() to convert the Path reference to an owned PathBuf
        Ok(repo.path().parent().unwrap_or(repo.path()).to_path_buf())
    }
    
    /// Cleans up temporary repositories
    ///
    /// Removes cloned repositories if keep_repository is false
    ///
    /// # Arguments
    /// * `repo_path` - Path to the repository
    ///
    /// # Returns
    /// Result indicating success or an error message
    pub fn cleanup(&self, repo_path: &Path) -> Result<(), String> {
        // Only remove the repository if keep_repository is false and the path exists
        if !self.keep_repository && repo_path.exists() {
            // Use std::fs::remove_dir_all to recursively remove the directory and its contents
            match std::fs::remove_dir_all(repo_path) {
                Ok(_) => Ok(()),
                Err(e) => Err(format!("Failed to remove repository: {}", e)),
            }
        } else {
            // If keep_repository is true or the path doesn't exist, just return Ok
            Ok(())
        }
    }
    
    /// Gets repository metadata
    ///
    /// # Arguments
    /// * `repo_path` - Path to the repository
    ///
    /// # Returns
    /// Result with repository metadata or an error
    pub fn get_repository_metadata(&self, repo_path: &Path) -> Result<RepositoryMetadata, String> {
        // Open the repository
        let repo = match git2::Repository::open(repo_path) {
            Ok(repo) => repo,
            Err(e) => return Err(format!("Failed to open repository: {}", e)),
        };
        
        // Get repository name from the path
        let name = repo_path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("unknown")
            .to_string();
        
        // Get current branch
        let head = match repo.head() {
            Ok(head) => head,
            Err(e) => return Err(format!("Failed to get HEAD: {}", e)),
        };
        
        let branch = match head.shorthand() {
            Some(name) => name.to_string(),
            None => "detached HEAD".to_string(),
        };
        
        // Count commits
        let mut revwalk = match repo.revwalk() {
            Ok(revwalk) => revwalk,
            Err(e) => return Err(format!("Failed to create revwalk: {}", e)),
        };
        
        // Configure revwalk to start from HEAD
        if let Err(e) = revwalk.push_head() {
            return Err(format!("Failed to push HEAD to revwalk: {}", e));
        }
        
        // Count commits
        let commit_count = match revwalk.count() {
            count => count,
            // If counting fails, return 0
        };
        
        // Get last commit date
        let last_commit = match repo.head().and_then(|head| head.peel_to_commit()) {
            Ok(commit) => commit,
            Err(e) => return Err(format!("Failed to get last commit: {}", e)),
        };
        
        let time = last_commit.time();
        let timestamp = time.seconds();
        // Use DateTime::from_timestamp instead of deprecated NaiveDateTime::from_timestamp_opt
        let last_commit_date = chrono::DateTime::<chrono::Utc>::from_timestamp(timestamp, 0)
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_else(|| "unknown".to_string());
        
        Ok(RepositoryMetadata {
            name,
            branch,
            commit_count,
            last_commit_date,
        })
    }
}

/// Represents Git repository metadata
pub struct RepositoryMetadata {
    /// Repository name
    pub name: String,
    
    /// Current branch
    pub branch: String,
    
    /// Number of commits
    pub commit_count: usize,
    
    /// Date of the last commit
    pub last_commit_date: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_validate_url() {
        // Valid URLs
        assert!(GitHandler::validate_url("https://github.com/user/repo.git"));
        assert!(GitHandler::validate_url("https://gitlab.com/user/repo.git"));
        assert!(GitHandler::validate_url("https://github.com/user/repo")); // Without .git suffix is valid
        assert!(GitHandler::validate_url("git@github.com:user/repo.git")); // SSH format is valid
        assert!(GitHandler::validate_url("file:///path/to/repo")); // Local file URL is valid
        assert!(GitHandler::validate_url("/absolute/path/to/repo")); // Absolute path is valid
        
        // Invalid URLs
        assert!(!GitHandler::validate_url("invalid-url")); // No protocol or path format
        assert!(!GitHandler::validate_url("ftp://github.com/user/repo.git")); // Unsupported protocol
    }
    
    #[test]
    fn test_new_git_handler() {
        let handler = GitHandler::new(true);
        assert!(handler.keep_repository);
        
        let handler = GitHandler::new(false);
        assert!(!handler.keep_repository);
    }
}