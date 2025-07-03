use crate::scanner::FileInfo;
use std::collections::HashMap;

#[derive(Default, Clone, Debug)]
pub struct RuleConfig {
    pub max_age_days: u64,
    pub min_size_mb: u64,
    pub max_size_mb: Option<u64>,
    pub file_types: Option<Vec<String>>,
    pub exclude_file_types: Option<Vec<String>>,
    pub include_hidden: bool,
    pub include_readonly: bool,
    pub include_executable: bool,
    pub custom_patterns: Vec<String>,
    pub exclude_patterns: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct SmartRule {
    pub name: String,
    pub description: String,
    pub config: RuleConfig,
    pub priority: u8,
}

impl SmartRule {
    pub fn new(name: &str, description: &str, config: RuleConfig) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            config,
            priority: 5,
        }
    }
}

pub fn apply_rules(files: &[FileInfo], rule: &RuleConfig) -> Vec<FileInfo> {
    let mut result = Vec::new();
    let max_age_secs = rule.max_age_days * 86400;
    let min_size_bytes = rule.min_size_mb * 1024 * 1024;
    let max_size_bytes = rule.max_size_mb.map(|mb| mb * 1024 * 1024);
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    
    for file in files {
        if !matches_rule(file, rule, max_age_secs, min_size_bytes, max_size_bytes, now_secs) {
            continue;
        }
        
        result.push(file.clone());
    }
    
    result
}

fn matches_rule(
    file: &FileInfo,
    rule: &RuleConfig,
    max_age_secs: u64,
    min_size_bytes: u64,
    max_size_bytes: Option<u64>,
    now_secs: u64,
) -> bool {
    // Age check - file.last_access_secs is a timestamp, so we need to calculate age
    let file_age_secs = now_secs.saturating_sub(file.last_access_secs);
    if file_age_secs < max_age_secs {
        return false;
    }
    
    // Size checks
    if file.size < min_size_bytes {
        return false;
    }
    
    if let Some(max_size) = max_size_bytes {
        if file.size > max_size {
            return false;
        }
    }
    
    // Hidden files check
    if file.is_hidden && !rule.include_hidden {
        return false;
    }
    
    // Readonly files check
    if file.is_readonly && !rule.include_readonly {
        return false;
    }
    
    // Executable files check
    if file.is_executable && !rule.include_executable {
        return false;
    }
    
    // File type inclusion filter
    if let Some(ref include_types) = rule.file_types {
        if !include_types.iter().any(|t| file.file_type.to_lowercase().contains(&t.to_lowercase())) {
            return false;
        }
    }
    
    // File type exclusion filter
    if let Some(ref exclude_types) = rule.exclude_file_types {
        if exclude_types.iter().any(|t| file.file_type.to_lowercase().contains(&t.to_lowercase())) {
            return false;
        }
    }
    
    // Custom pattern matching
    if !rule.custom_patterns.is_empty() {
        let matches_pattern = rule.custom_patterns.iter().any(|pattern| {
            pattern_matches(&file.path, pattern)
        });
        if !matches_pattern {
            return false;
        }
    }
    
    // Exclude pattern matching
    if !rule.exclude_patterns.is_empty() {
        let matches_exclude = rule.exclude_patterns.iter().any(|pattern| {
            pattern_matches(&file.path, pattern)
        });
        if matches_exclude {
            return false;
        }
    }
    
    true
}

fn pattern_matches(path: &str, pattern: &str) -> bool {
    // Simple pattern matching with wildcards
    if pattern.contains('*') {
        wildcard_match(pattern, path)
    } else {
        path.to_lowercase().contains(&pattern.to_lowercase())
    }
}

fn wildcard_match(pattern: &str, text: &str) -> bool {
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
    let text_lower = text.to_lowercase();
    
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        
        let part_lower = part.to_lowercase();
        
        if i == 0 {
            if !text_lower.starts_with(&part_lower) {
                return false;
            }
            text_pos = part_lower.len();
        } else if i == parts.len() - 1 {
            return text_lower[text_pos..].ends_with(&part_lower);
        } else {
            if let Some(pos) = text_lower[text_pos..].find(&part_lower) {
                text_pos += pos + part_lower.len();
            } else {
                return false;
            }
        }
    }
    
    true
}

// Predefined smart rules
pub fn get_predefined_rules() -> Vec<SmartRule> {
    vec![
        SmartRule::new(
            "Large Old Downloads",
            "Files in Downloads folder older than 30 days and larger than 10MB",
            RuleConfig {
                max_age_days: 30,
                min_size_mb: 10,
                custom_patterns: vec!["*/Downloads/*".to_string(), "*/downloads/*".to_string()],
                ..Default::default()
            }
        ),
        SmartRule::new(
            "Temporary Files",
            "Common temporary files and cache",
            RuleConfig {
                max_age_days: 7,
                min_size_mb: 1,
                file_types: Some(vec!["tmp".to_string(), "temp".to_string(), "cache".to_string()]),
                ..Default::default()
            }
        ),
        SmartRule::new(
            "Old Media Files",
            "Large media files not accessed in 90 days",
            RuleConfig {
                max_age_days: 90,
                min_size_mb: 50,
                file_types: Some(vec!["Video".to_string(), "Audio".to_string(), "Image".to_string()]),
                ..Default::default()
            }
        ),
        SmartRule::new(
            "Huge Files",
            "Files larger than 500MB regardless of age",
            RuleConfig {
                max_age_days: 0,
                min_size_mb: 500,
                ..Default::default()
            }
        ),
        SmartRule::new(
            "Old Archives",
            "Archive files older than 180 days",
            RuleConfig {
                max_age_days: 180,
                min_size_mb: 1,
                file_types: Some(vec!["Archive".to_string()]),
                ..Default::default()
            }
        ),
        SmartRule::new(
            "Old Documents",
            "Document files not accessed in 365 days",
            RuleConfig {
                max_age_days: 365,
                min_size_mb: 1,
                file_types: Some(vec!["Document".to_string(), "PDF".to_string(), "Spreadsheet".to_string()]),
                ..Default::default()
            }
        ),
        SmartRule::new(
            "Log Files",
            "Log files older than 30 days",
            RuleConfig {
                max_age_days: 30,
                min_size_mb: 1,
                custom_patterns: vec!["*.log".to_string(), "*.log.*".to_string()],
                ..Default::default()
            }
        ),
        SmartRule::new(
            "Backup Files",
            "Backup files older than 60 days",
            RuleConfig {
                max_age_days: 60,
                min_size_mb: 1,
                custom_patterns: vec!["*.bak".to_string(), "*.backup".to_string(), "*~".to_string()],
                ..Default::default()
            }
        ),
    ]
}

pub fn analyze_file_patterns(files: &[FileInfo]) -> HashMap<String, Vec<String>> {
    let mut patterns = HashMap::new();
    
    // Analyze by file type
    let mut by_type: HashMap<String, Vec<String>> = HashMap::new();
    for file in files {
        by_type.entry(file.file_type.clone())
            .or_insert_with(Vec::new)
            .push(file.path.clone());
    }
    patterns.insert("by_type".to_string(), 
        by_type.into_iter().map(|(k, v)| format!("{}: {} files", k, v.len())).collect());
    
    // Analyze by directory
    let mut by_dir: HashMap<String, usize> = HashMap::new();
    for file in files {
        if let Some(parent) = std::path::Path::new(&file.path).parent() {
            let dir = parent.to_str().unwrap_or("unknown");
            *by_dir.entry(dir.to_string()).or_insert(0) += 1;
        }
    }
    let mut dir_counts: Vec<_> = by_dir.into_iter().collect();
    dir_counts.sort_by(|a, b| b.1.cmp(&a.1));
    patterns.insert("by_directory".to_string(),
        dir_counts.into_iter().take(10).map(|(k, v)| format!("{}: {} files", k, v)).collect());
    
    // Analyze by size ranges
    let mut size_ranges = HashMap::new();
    for file in files {
        let range = match file.size {
            0..=1024 => "0-1KB",
            1025..=1048576 => "1KB-1MB",
            1048577..=104857600 => "1MB-100MB",
            104857601..=1073741824 => "100MB-1GB",
            _ => "1GB+",
        };
        *size_ranges.entry(range.to_string()).or_insert(0) += 1;
    }
    patterns.insert("by_size".to_string(),
        size_ranges.into_iter().map(|(k, v)| format!("{}: {} files", k, v)).collect());
    
    patterns
}

pub fn suggest_rules_for_files(files: &[FileInfo]) -> Vec<SmartRule> {
    let mut suggestions = Vec::new();
    
    // Calculate statistics
    let total_size: u64 = files.iter().map(|f| f.size).sum();
    let avg_size = if !files.is_empty() { total_size / files.len() as u64 } else { 0 };
    
    // Suggest rules based on file patterns
    let mut type_counts: HashMap<String, usize> = HashMap::new();
    for file in files {
        *type_counts.entry(file.file_type.clone()).or_insert(0) += 1;
    }
    
    // Suggest rule for most common file type if it's significant
    if let Some((most_common_type, count)) = type_counts.iter().max_by_key(|(_, &count)| count) {
        if *count > files.len() / 4 {  // If more than 25% of files are of this type
            suggestions.push(SmartRule::new(
                &format!("Clean {} Files", most_common_type),
                &format!("Target {} files which make up a large portion of your files", most_common_type),
                RuleConfig {
                    max_age_days: 60,
                    min_size_mb: (avg_size / (1024 * 1024)).max(1),
                    file_types: Some(vec![most_common_type.clone()]),
                    ..Default::default()
                }
            ));
        }
    }
    
    // Suggest rule for very large files
    let large_files: Vec<_> = files.iter().filter(|f| f.size > 100 * 1024 * 1024).collect();
    if !large_files.is_empty() {
        suggestions.push(SmartRule::new(
            "Large File Cleanup",
            "Focus on files larger than 100MB",
            RuleConfig {
                max_age_days: 30,
                min_size_mb: 100,
                ..Default::default()
            }
        ));
    }
    
    // Suggest rule for very old files
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let very_old_files: Vec<_> = files.iter().filter(|f| {
        let file_age_secs = now_secs.saturating_sub(f.last_access_secs);
        file_age_secs > 365 * 86400
    }).collect();
    if !very_old_files.is_empty() {
        suggestions.push(SmartRule::new(
            "Ancient Files",
            "Files not accessed in over a year",
            RuleConfig {
                max_age_days: 365,
                min_size_mb: 1,
                ..Default::default()
            }
        ));
    }
    
    suggestions
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
    fn test_pattern_matches() {
        assert!(pattern_matches("/home/user/Downloads/file.txt", "*/Downloads/*"));
        assert!(pattern_matches("/home/user/file.log", "*.log"));
        assert!(!pattern_matches("/home/user/file.txt", "*.log"));
    }
}