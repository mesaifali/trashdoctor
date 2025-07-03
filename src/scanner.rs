use walkdir::WalkDir;
use std::fs;
use chrono::{DateTime, Local};
use std::time::SystemTime;
use std::path::Path;
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct FileInfo {
    pub path: String,
    pub size: u64,
    pub last_accessed: String,
    pub last_access_secs: u64,
    pub last_modified: String,
    pub last_modified_secs: u64,
    pub file_type: String,
    pub is_hidden: bool,
    pub is_readonly: bool,
    pub is_executable: bool,
}

#[derive(Clone, Debug)]
pub struct ScanOptions {
    pub include_hidden: bool,
    pub include_system: bool,
    pub max_depth: Option<usize>,
    pub follow_symlinks: bool,
    pub file_extensions: Option<Vec<String>>,
    pub exclude_patterns: Vec<String>,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            include_hidden: false,
            include_system: false,
            max_depth: None,
            follow_symlinks: false,
            file_extensions: None,
            exclude_patterns: vec![
                "*.tmp".to_string(),
                "*.cache".to_string(),
                "*/.git/*".to_string(),
                "*/node_modules/*".to_string(),
            ],
        }
    }
}

pub fn scan_folder(folder: &str) -> Vec<FileInfo> {
    scan_folder_with_options(folder, &ScanOptions::default())
}

pub fn scan_folder_with_options(folder: &str, options: &ScanOptions) -> Vec<FileInfo> {
    let mut files = Vec::new();
    
    let mut walker = WalkDir::new(folder).follow_links(options.follow_symlinks);
    
    if let Some(max_depth) = options.max_depth {
        walker = walker.max_depth(max_depth);
    }
    
    for entry in walker.into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            let path = entry.path();
            
            // Skip hidden files if not requested
            if !options.include_hidden && is_hidden_file(path) {
                continue;
            }
            
            // Check exclude patterns
            if should_exclude_file(path, &options.exclude_patterns) {
                continue;
            }
            
            // Check file extension filter
            if let Some(ref extensions) = options.file_extensions {
                if let Some(ext) = path.extension() {
                    let ext_str = ext.to_str().unwrap_or("").to_lowercase();
                    if !extensions.iter().any(|e| e.to_lowercase() == ext_str) {
                        continue;
                    }
                } else if !extensions.is_empty() {
                    continue;
                }
            }
            
            if let Ok(metadata) = fs::metadata(path) {
                let accessed = metadata.accessed().unwrap_or(SystemTime::now());
                let modified = metadata.modified().unwrap_or(SystemTime::now());
                
                let access_datetime: DateTime<Local> = accessed.into();
                let modified_datetime: DateTime<Local> = modified.into();
                
                let access_age_secs = accessed.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
                let modified_age_secs = modified.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
                
                let file_type = get_file_type_from_path(path);
                let is_hidden = is_hidden_file(path);
                let is_readonly = metadata.permissions().readonly();
                let is_executable = is_executable_file(&metadata);
                
                files.push(FileInfo {
                    path: path.display().to_string(),
                    size: metadata.len(),
                    last_accessed: access_datetime.format("%Y-%m-%d %H:%M:%S").to_string(),
                    last_access_secs: access_age_secs,
                    last_modified: modified_datetime.format("%Y-%m-%d %H:%M:%S").to_string(),
                    last_modified_secs: modified_age_secs,
                    file_type,
                    is_hidden,
                    is_readonly,
                    is_executable,
                });
            }
        }
    }
    
    files
}

pub fn get_file_type_statistics(files: &[FileInfo]) -> HashMap<String, (usize, u64)> {
    let mut stats = HashMap::new();
    
    for file in files {
        let entry = stats.entry(file.file_type.clone()).or_insert((0, 0));
        entry.0 += 1;  // count
        entry.1 += file.size;  // total size
    }
    
    stats
}

pub fn get_largest_files(files: &[FileInfo], count: usize) -> Vec<&FileInfo> {
    let mut sorted_files: Vec<&FileInfo> = files.iter().collect();
    sorted_files.sort_by(|a, b| b.size.cmp(&a.size));
    sorted_files.into_iter().take(count).collect()
}

pub fn get_oldest_files(files: &[FileInfo], count: usize) -> Vec<&FileInfo> {
    let mut sorted_files: Vec<&FileInfo> = files.iter().collect();
    sorted_files.sort_by(|a, b| b.last_access_secs.cmp(&a.last_access_secs));
    sorted_files.into_iter().take(count).collect()
}

pub fn get_duplicate_files(files: &[FileInfo]) -> HashMap<u64, Vec<&FileInfo>> {
    let mut size_groups: HashMap<u64, Vec<&FileInfo>> = HashMap::new();
    
    for file in files {
        size_groups.entry(file.size).or_insert_with(Vec::new).push(file);
    }
    
    // Filter to only groups with more than one file
    size_groups.into_iter()
        .filter(|(_, files)| files.len() > 1)
        .collect()
}

pub fn calculate_space_savings(files: &[FileInfo]) -> (u64, u64) {
    let duplicates = get_duplicate_files(files);
    let mut potential_savings = 0u64;
    let mut duplicate_count = 0u64;
    
    for (size, duplicate_files) in duplicates {
        if duplicate_files.len() > 1 {
            // Keep one copy, remove the rest
            potential_savings += size * (duplicate_files.len() - 1) as u64;
            duplicate_count += (duplicate_files.len() - 1) as u64;
        }
    }
    
    (potential_savings, duplicate_count)
}

fn get_file_type_from_path(path: &Path) -> String {
    match path.extension() {
        Some(ext) => {
            let ext_str = ext.to_str().unwrap_or("unknown").to_lowercase();
            match ext_str.as_str() {
                "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" | "svg" => "Image".to_string(),
                "mp4" | "avi" | "mov" | "mkv" | "flv" | "wmv" | "webm" => "Video".to_string(),
                "mp3" | "wav" | "flac" | "aac" | "ogg" | "m4a" => "Audio".to_string(),
                "pdf" => "PDF".to_string(),
                "doc" | "docx" | "odt" => "Document".to_string(),
                "xls" | "xlsx" | "ods" => "Spreadsheet".to_string(),
                "ppt" | "pptx" | "odp" => "Presentation".to_string(),
                "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" => "Archive".to_string(),
                "exe" | "msi" | "deb" | "rpm" | "dmg" | "pkg" => "Executable".to_string(),
                "txt" | "md" | "log" | "cfg" | "ini" | "conf" => "Text".to_string(),
                "html" | "htm" | "css" | "js" | "json" | "xml" => "Web".to_string(),
                "c" | "cpp" | "h" | "py" | "java" | "rs" | "go" => "Code".to_string(),
                _ => format!(".{}", ext_str),
            }
        }
        None => "No Extension".to_string(),
    }
}

fn is_hidden_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.starts_with('.'))
        .unwrap_or(false)
}

fn should_exclude_file(path: &Path, exclude_patterns: &[String]) -> bool {
    let path_str = path.to_str().unwrap_or("");
    
    for pattern in exclude_patterns {
        if pattern.contains('*') {
            // Simple wildcard matching
            if wildcard_match(pattern, path_str) {
                return true;
            }
        } else if path_str.contains(pattern) {
            return true;
        }
    }
    
    false
}

fn wildcard_match(pattern: &str, text: &str) -> bool {
    // Simple wildcard matching - supports * only
    if pattern == "*" {
        return true;
    }
    
    if !pattern.contains('*') {
        return pattern == text;
    }
    
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.is_empty() {
        return true;
    }
    
    let mut text_pos = 0;
    
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        
        if i == 0 {
            // First part must match at the beginning
            if !text.starts_with(part) {
                return false;
            }
            text_pos = part.len();
        } else if i == parts.len() - 1 {
            // Last part must match at the end
            return text[text_pos..].ends_with(part);
        } else {
            // Middle parts must be found in order
            if let Some(pos) = text[text_pos..].find(part) {
                text_pos += pos + part.len();
            } else {
                return false;
            }
        }
    }
    
    true
}

#[cfg(unix)]
fn is_executable_file(metadata: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    let permissions = metadata.permissions();
    permissions.mode() & 0o111 != 0
}

#[cfg(windows)]
fn is_executable_file(metadata: &std::fs::Metadata) -> bool {
    // On Windows, we can't easily check execute permissions like on Unix
    // For now, just return false
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_wildcard_match() {
        assert!(wildcard_match("*.txt", "test.txt"));
        assert!(wildcard_match("test*", "test.txt"));
        assert!(wildcard_match("*test*", "mytest.txt"));
        assert!(!wildcard_match("*.jpg", "test.txt"));
    }
    
    #[test]
    fn test_get_file_type() {
        assert_eq!(get_file_type_from_path(Path::new("test.jpg")), "Image");
        assert_eq!(get_file_type_from_path(Path::new("test.mp4")), "Video");
        assert_eq!(get_file_type_from_path(Path::new("test.unknown")), ".unknown");
        assert_eq!(get_file_type_from_path(Path::new("test")), "No Extension");
    }
    
    #[test]
    fn test_is_hidden_file() {
        assert!(is_hidden_file(Path::new(".hidden")));
        assert!(!is_hidden_file(Path::new("visible.txt")));
    }
}