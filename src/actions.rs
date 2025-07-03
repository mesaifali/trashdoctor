use std::fs;
use std::path::Path;
use std::env;
use std::fs::{create_dir_all, copy};
use std::io::{self, ErrorKind};

#[derive(Debug)]
pub enum FileActionError {
    PermissionDenied,
    FileNotFound,
    InsufficientSpace,
    FileInUse,
    Other(String),
}

impl From<io::Error> for FileActionError {
    fn from(error: io::Error) -> Self {
        match error.kind() {
            ErrorKind::PermissionDenied => FileActionError::PermissionDenied,
            ErrorKind::NotFound => FileActionError::FileNotFound,
            ErrorKind::Other => {
                if let Some(raw_os_error) = error.raw_os_error() {
                    match raw_os_error {
                        28 => FileActionError::InsufficientSpace, // ENOSPC
                        16 => FileActionError::FileInUse,         // EBUSY
                        _ => FileActionError::Other(error.to_string()),
                    }
                } else {
                    FileActionError::Other(error.to_string())
                }
            }
            _ => FileActionError::Other(error.to_string()),
        }
    }
}

impl std::fmt::Display for FileActionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FileActionError::PermissionDenied => write!(f, "Permission denied"),
            FileActionError::FileNotFound => write!(f, "File not found"),
            FileActionError::InsufficientSpace => write!(f, "Insufficient disk space"),
            FileActionError::FileInUse => write!(f, "File is currently in use"),
            FileActionError::Other(msg) => write!(f, "Error: {}", msg),
        }
    }
}

pub fn delete_file(path: &str) -> Result<(), FileActionError> {
    // Check if file exists first
    if !Path::new(path).exists() {
        return Err(FileActionError::FileNotFound);
    }

    // Check if we have permission to delete
    let metadata = fs::metadata(path)?;
    if metadata.permissions().readonly() {
        return Err(FileActionError::PermissionDenied);
    }

    // Try to delete the file
    fs::remove_file(path)?;
    Ok(())
}

pub fn archive_file(path: &str) -> Result<(), FileActionError> {
    // Check if file exists
    if !Path::new(path).exists() {
        return Err(FileActionError::FileNotFound);
    }

    // Get home directory
    let home = env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let archive_dir = format!("{}/.trashdoctor/archive", home);
    
    // Create archive directory if it doesn't exist
    create_dir_all(&archive_dir)?;

    // Get filename and create unique archive path
    let filename = Path::new(path)
        .file_name()
        .ok_or_else(|| FileActionError::Other("Invalid file path".to_string()))?
        .to_str()
        .ok_or_else(|| FileActionError::Other("Invalid filename encoding".to_string()))?;

    let mut archive_path = format!("{}/{}", archive_dir, filename);
    let mut counter = 1;
    
    // Handle duplicate filenames by adding a counter
    while Path::new(&archive_path).exists() {
        let stem = Path::new(filename).file_stem().unwrap_or_default().to_str().unwrap_or("");
        let ext = Path::new(filename).extension().unwrap_or_default().to_str().unwrap_or("");
        if ext.is_empty() {
            archive_path = format!("{}/{}_{}", archive_dir, stem, counter);
        } else {
            archive_path = format!("{}/{}_{}.{}", archive_dir, stem, counter, ext);
        }
        counter += 1;
    }

    // Copy file to archive
    copy(path, &archive_path)?;
    
    // Delete original file
    delete_file(path)?;
    
    Ok(())
}

pub fn move_to_trash(path: &str) -> Result<(), FileActionError> {
    // Use system trash if available
    #[cfg(feature = "trash")]
    {
        use trash::delete;
        delete(path).map_err(|e| FileActionError::Other(e.to_string()))?;
        Ok(())
    }
    
    #[cfg(not(feature = "trash"))]
    {
        // Fallback to manual trash implementation
        let home = env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let trash_dir = format!("{}/.local/share/Trash/files", home);
        let trash_info_dir = format!("{}/.local/share/Trash/info", home);
        
        create_dir_all(&trash_dir)?;
        create_dir_all(&trash_info_dir)?;
        
        let filename = Path::new(path)
            .file_name()
            .ok_or_else(|| FileActionError::Other("Invalid file path".to_string()))?
            .to_str()
            .ok_or_else(|| FileActionError::Other("Invalid filename encoding".to_string()))?;
        
        let mut trash_path = format!("{}/{}", trash_dir, filename);
        let mut counter = 1;
        
        // Handle duplicate filenames
        while Path::new(&trash_path).exists() {
            let stem = Path::new(filename).file_stem().unwrap_or_default().to_str().unwrap_or("");
            let ext = Path::new(filename).extension().unwrap_or_default().to_str().unwrap_or("");
            if ext.is_empty() {
                trash_path = format!("{}/{}_{}", trash_dir, stem, counter);
            } else {
                trash_path = format!("{}/{}_{}.{}", trash_dir, stem, counter, ext);
            }
            counter += 1;
        }
        
        // Move file to trash
        fs::rename(path, &trash_path)?;
        
        // Create .trashinfo file
        let trash_info_path = format!("{}/{}.trashinfo", trash_info_dir, 
            Path::new(&trash_path).file_name().unwrap().to_str().unwrap());
        
        let deletion_date = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
        let trash_info_content = format!(
            "[Trash Info]\nPath={}\nDeletionDate={}\n",
            path, deletion_date
        );
        
        fs::write(&trash_info_path, trash_info_content)?;
        
        Ok(())
    }
}

pub fn get_file_size(path: &str) -> Result<u64, FileActionError> {
    let metadata = fs::metadata(path)?;
    Ok(metadata.len())
}

pub fn is_file_writable(path: &str) -> Result<bool, FileActionError> {
    let metadata = fs::metadata(path)?;
    Ok(!metadata.permissions().readonly())
}

pub fn get_file_type(path: &str) -> String {
    Path::new(path)
        .extension()
        .unwrap_or_default()
        .to_str()
        .unwrap_or("unknown")
        .to_lowercase()
}

pub fn format_file_size(size: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size_f = size as f64;
    let mut unit_idx = 0;
    
    while size_f >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size_f /= 1024.0;
        unit_idx += 1;
    }
    
    if unit_idx == 0 {
        format!("{} {}", size, UNITS[unit_idx])
    } else {
        format!("{:.2} {}", size_f, UNITS[unit_idx])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_format_file_size() {
        assert_eq!(format_file_size(512), "512 B");
        assert_eq!(format_file_size(1024), "1.00 KB");
        assert_eq!(format_file_size(1048576), "1.00 MB");
        assert_eq!(format_file_size(1073741824), "1.00 GB");
    }
    
    #[test]
    fn test_get_file_type() {
        assert_eq!(get_file_type("test.txt"), "txt");
        assert_eq!(get_file_type("test.PDF"), "pdf");
        assert_eq!(get_file_type("test"), "unknown");
    }
}