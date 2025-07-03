mod scanner;
mod rules;
mod actions;

use iced::{Application, Command, Element, executor, Settings, Theme, Length, widget::{column, row, scrollable, text, button, checkbox, text_input, container, progress_bar}, theme};
use scanner::scan_folder;
use scanner::FileInfo;
use rules::{apply_rules, RuleConfig};
use actions::{delete_file, archive_file};
use rfd::FileDialog;
use std::time::Duration;

#[derive(Debug, Clone)]
enum Message {
    ToggleSelection(usize, bool),
    DeleteSelected,
    ArchiveSelected,
    FolderSelected(String),
    ChangeAge(String),
    ChangeSize(String),
    Refresh,
    SelectFolder,
    SelectAll(bool),
    ConfirmDelete,
    CancelDelete,
    ShowDeleteConfirmation,
    SortBy(SortCriteria),
    FilterByType(String),
    ClearMessage,
    PreviewFile(String),
    ShowStats,
    ExportList,
    ToggleAutoRefresh(bool),
    AutoRefreshTick,
}

#[derive(Debug, Clone)]
enum SortCriteria {
    Name,
    Size,
    Date,
    Type,
}

#[derive(Debug, Clone)]
enum AppState {
    Normal,
    ConfirmingDelete,
    Processing,
}

pub fn main() -> iced::Result {
    TrashDoctor::run(Settings::default())
}

struct TrashDoctor {
    files: Vec<FileInfo>,
    all_files: Vec<FileInfo>, // Store all files for filtering
    selected: Vec<bool>,
    message: String,
    message_type: MessageType,
    folder_path: String,
    age_filter: String,
    size_filter: String,
    rule: RuleConfig,
    state: AppState,
    sort_by: SortCriteria,
    filter_by_type: String,
    auto_refresh: bool,
    stats: FileStats,
    selected_count: usize,
    total_size_selected: u64,
}

#[derive(Debug, Clone)]
enum MessageType {
    Info,
    Success,
    Error,
    Warning,
}

#[derive(Debug, Clone, Default)]
struct FileStats {
    total_files: usize,
    total_size: u64,
    oldest_file: String,
    newest_file: String,
    largest_file: String,
    file_types: std::collections::HashMap<String, usize>,
}

impl Application for TrashDoctor {
    type Executor = executor::Default;
    type Message = Message;
    type Theme = Theme;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Message>) {
        let folder = String::from("/home");
        (
            TrashDoctor {
                files: vec![],
                all_files: vec![],
                selected: vec![],
                message: "Select a folder to begin scanning for old files.".to_string(),
                message_type: MessageType::Info,
                folder_path: folder,
                age_filter: "30".into(),
                size_filter: "100".into(),
                rule: RuleConfig::default(),
                state: AppState::Normal,
                sort_by: SortCriteria::Date,
                filter_by_type: "All".to_string(),
                auto_refresh: false,
                stats: FileStats::default(),
                selected_count: 0,
                total_size_selected: 0,
            },
            Command::none(),
        )
    }

    fn title(&self) -> String {
        String::from("TrashDoctor - Smart Disk Hygiene & File Management")
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::ToggleSelection(index, value) => {
                if index < self.selected.len() {
                    self.selected[index] = value;
                    self.update_selection_stats();
                }
            }
            Message::SelectAll(value) => {
                self.selected = vec![value; self.files.len()];
                self.update_selection_stats();
            }
            Message::ShowDeleteConfirmation => {
                if self.selected_count > 0 {
                    self.state = AppState::ConfirmingDelete;
                    self.message = format!("Are you sure you want to delete {} files? This action cannot be undone!", self.selected_count);
                    self.message_type = MessageType::Warning;
                } else {
                    self.message = "No files selected for deletion.".to_string();
                    self.message_type = MessageType::Warning;
                }
            }
            Message::ConfirmDelete => {
                self.state = AppState::Processing;
                let mut deleted_count = 0;
                let mut failed_count = 0;
                
                for (i, selected) in self.selected.iter().enumerate() {
                    if *selected && i < self.files.len() {
                        match delete_file(&self.files[i].path) {
                            Ok(_) => deleted_count += 1,
                            Err(_) => failed_count += 1,
                        }
                    }
                }
                
                self.state = AppState::Normal;
                if failed_count == 0 {
                    self.message = format!("Successfully deleted {} files.", deleted_count);
                    self.message_type = MessageType::Success;
                } else {
                    self.message = format!("Deleted {} files, failed to delete {} files.", deleted_count, failed_count);
                    self.message_type = MessageType::Error;
                }
                
                return Command::perform(async {}, |_| Message::Refresh);
            }
            Message::CancelDelete => {
                self.state = AppState::Normal;
                self.message = "Delete operation cancelled.".to_string();
                self.message_type = MessageType::Info;
            }
            Message::ArchiveSelected => {
                if self.selected_count == 0 {
                    self.message = "No files selected for archiving.".to_string();
                    self.message_type = MessageType::Warning;
                    return Command::none();
                }
                
                self.state = AppState::Processing;
                let mut archived_count = 0;
                let mut failed_count = 0;
                
                for (i, selected) in self.selected.iter().enumerate() {
                    if *selected && i < self.files.len() {
                        match archive_file(&self.files[i].path) {
                            Ok(_) => archived_count += 1,
                            Err(_) => failed_count += 1,
                        }
                    }
                }
                
                self.state = AppState::Normal;
                if failed_count == 0 {
                    self.message = format!("Successfully archived {} files.", archived_count);
                    self.message_type = MessageType::Success;
                } else {
                    self.message = format!("Archived {} files, failed to archive {} files.", archived_count, failed_count);
                    self.message_type = MessageType::Error;
                }
                
                return Command::perform(async {}, |_| Message::Refresh);
            }
            Message::FolderSelected(path) => {
                if !path.is_empty() {
                    self.folder_path = path.clone();
                    self.scan_and_filter();
                }
            }
            Message::ChangeAge(age) => {
                self.age_filter = age;
                if !self.folder_path.is_empty() {
                    self.scan_and_filter();
                }
            }
            Message::ChangeSize(size) => {
                self.size_filter = size;
                if !self.folder_path.is_empty() {
                    self.scan_and_filter();
                }
            }
            Message::Refresh => {
                if !self.folder_path.is_empty() {
                    self.scan_and_filter();
                    self.message = "Files refreshed successfully.".to_string();
                    self.message_type = MessageType::Success;
                } else {
                    self.message = "No folder selected. Please select a folder first.".to_string();
                    self.message_type = MessageType::Warning;
                }
            }
            Message::SelectFolder => {
                return Command::perform(
                    async move {
                        if let Some(folder) = FileDialog::new().pick_folder() {
                            Message::FolderSelected(folder.display().to_string())
                        } else {
                            Message::FolderSelected("".into())
                        }
                    },
                    |msg| msg,
                );
            }
            Message::SortBy(criteria) => {
                self.sort_by = criteria;
                self.apply_sort_and_filter();
            }
            Message::FilterByType(file_type) => {
                self.filter_by_type = file_type;
                self.apply_sort_and_filter();
            }
            Message::ClearMessage => {
                self.message = "Ready.".to_string();
                self.message_type = MessageType::Info;
            }
            Message::PreviewFile(path) => {
                self.message = format!("Preview: {}", path);
                self.message_type = MessageType::Info;
            }
            Message::ShowStats => {
                let stats_text = format!(
                    "Total Files: {}, Total Size: {:.2} MB, Selected: {} files ({:.2} MB)",
                    self.stats.total_files,
                    self.stats.total_size as f64 / (1024.0 * 1024.0),
                    self.selected_count,
                    self.total_size_selected as f64 / (1024.0 * 1024.0)
                );
                self.message = stats_text;
                self.message_type = MessageType::Info;
            }
            Message::ExportList => {
                self.message = "Export functionality not implemented yet.".to_string();
                self.message_type = MessageType::Info;
            }
            Message::ToggleAutoRefresh(value) => {
                self.auto_refresh = value;
                if self.auto_refresh {
                    self.message = "Auto-refresh enabled (every 30 seconds).".to_string();
                    self.message_type = MessageType::Success;
                    return Command::perform(
                        async { tokio::time::sleep(Duration::from_secs(30)).await },
                        |_| Message::AutoRefreshTick,
                    );
                } else {
                    self.message = "Auto-refresh disabled.".to_string();
                    self.message_type = MessageType::Info;
                }
            }
            Message::AutoRefreshTick => {
                if self.auto_refresh {
                    self.scan_and_filter();
                    return Command::perform(
                        async { tokio::time::sleep(Duration::from_secs(30)).await },
                        |_| Message::AutoRefreshTick,
                    );
                }
            }
            Message::DeleteSelected => {
                return Command::perform(async {}, |_| Message::ShowDeleteConfirmation);
            }
        }
        Command::none()
    }

    fn view(&self) -> Element<Message> {
        let header = text("TrashDoctor - Smart Disk Hygiene & File Management")
            .size(24);

        // Controls section
        let controls = column![
            row![
                text("Folder:").width(Length::Fixed(60.0)),
                button(" Select Folder").on_press(Message::SelectFolder),
                text(&self.folder_path).width(Length::Fill),
                button(" Refresh").on_press(Message::Refresh),
                checkbox("Auto-refresh", self.auto_refresh, Message::ToggleAutoRefresh),
            ]
            .spacing(10)
            .align_items(iced::Alignment::Center),

            row![
                text("Max Age (days):").width(Length::Fixed(120.0)),
                text_input("30", &self.age_filter)
                    .on_input(Message::ChangeAge)
                    .width(Length::Fixed(80.0)),

                text("Min Size (MB):").width(Length::Fixed(120.0)),
                text_input("100", &self.size_filter)
                    .on_input(Message::ChangeSize)
                    .width(Length::Fixed(80.0)),

                text("Sort by:").width(Length::Fixed(80.0)),
                button("Date").on_press(Message::SortBy(SortCriteria::Date)),
                button("Size").on_press(Message::SortBy(SortCriteria::Size)),
                button("Name").on_press(Message::SortBy(SortCriteria::Name)),
                button("Type").on_press(Message::SortBy(SortCriteria::Type)),
            ]
            .spacing(10)
            .align_items(iced::Alignment::Center),

            row![
                text("Filter:").width(Length::Fixed(60.0)),
                button("All").on_press(Message::FilterByType("All".to_string())),
                button("Images").on_press(Message::FilterByType("Images".to_string())),
                button("Documents").on_press(Message::FilterByType("Documents".to_string())),
                button("Videos").on_press(Message::FilterByType("Videos".to_string())),
                button("Show Stats").on_press(Message::ShowStats),
                button("Export List").on_press(Message::ExportList),
                button("Clear Message").on_press(Message::ClearMessage),
            ]
            .spacing(10)
        ]
        .spacing(15)
        .padding(10);

        // Selection controls
        let selection_controls = row![
            checkbox("Select All", self.selected.iter().all(|&x| x), Message::SelectAll),
            text(format!("Selected: {} files ({:.2} MB)", 
                self.selected_count, 
                self.total_size_selected as f64 / (1024.0 * 1024.0)
            )).width(Length::Fill),
        ]
        .spacing(10)
        .align_items(iced::Alignment::Center)
        .padding(10);

        // File list header
        let file_list_header = container(
            row![
                text("Select").width(Length::Fixed(60.0)),
                text("File Path").width(Length::FillPortion(5)),
                text("Size (KB)").width(Length::Fixed(100.0)),
                text("Last Accessed").width(Length::Fixed(120.0)),
                text("Actions").width(Length::Fixed(80.0)),
            ]
            .padding(5)
        )
        .style(theme::Container::Custom(Box::new(HeaderStyle)));

        // File list rows
        let file_list = self.files.iter().enumerate().fold(
            column![file_list_header],
            |col, (i, file)| {
                let row_style = if i % 2 == 0 {
                    theme::Container::Custom(Box::new(EvenRowStyle))
                } else {
                    theme::Container::Custom(Box::new(OddRowStyle))
                };
                
                col.push(
                    container(
                        row![
                            checkbox("", self.selected.get(i).copied().unwrap_or(false), move |val| Message::ToggleSelection(i, val))
                                .width(Length::Fixed(60.0)),
                            text(&file.path).width(Length::FillPortion(5)),
                            text(format!("{}", file.size / 1024)).width(Length::Fixed(100.0)),
                            text(&file.last_accessed).width(Length::Fixed(120.0)),
                            button("👁").on_press(Message::PreviewFile(file.path.clone())).width(Length::Fixed(80.0)),
                        ]
                        .padding(5)
                        .spacing(5)
                        .align_items(iced::Alignment::Center)
                    )
                    .style(row_style)
                )
            },
        );

        // Actions row
        let actions = match self.state {
            AppState::ConfirmingDelete => {
                row![
                    button("Confirm Delete").on_press(Message::ConfirmDelete),
                    button("Cancel").on_press(Message::CancelDelete),
                ]
                .spacing(20)
                .padding(10)
            }
            AppState::Processing => {
                row![
                    text("Processing..."),
                    progress_bar(0.0..=100.0, 50.0),
                ]
                .spacing(20)
                .padding(10)
            }
            AppState::Normal => {
                row![
                    button("Delete Selected").on_press(Message::DeleteSelected),
                    button("Archive Selected").on_press(Message::ArchiveSelected),
                ]
                .spacing(20)
                .padding(10)
            }
        };

        // Status message with color coding
        let status_color = match self.message_type {
            MessageType::Success => iced::Color::from_rgb(0.0, 0.7, 0.0),
            MessageType::Error => iced::Color::from_rgb(0.8, 0.0, 0.0),
            MessageType::Warning => iced::Color::from_rgb(0.8, 0.5, 0.0),
            MessageType::Info => iced::Color::from_rgb(0.0, 0.0, 0.8),
        };
        
        let status = text(&self.message).size(14).style(status_color);

        // File count and size summary
        let summary = text(format!(
            "Total: {} files ({:.2} MB) | Filtered: {} files", 
            self.all_files.len(),
            self.all_files.iter().map(|f| f.size).sum::<u64>() as f64 / (1024.0 * 1024.0),
            self.files.len()
        )).size(12);

        // Compose layout
        column![
            header,
            controls,
            selection_controls,
            scrollable(file_list).height(Length::FillPortion(1)),
            actions,
            status,
            summary,
        ]
        .spacing(15)
        .padding(15)
        .into()
    }
}

impl TrashDoctor {
    fn scan_and_filter(&mut self) {
        self.all_files = scan_folder(&self.folder_path);
        self.rule.max_age_days = self.age_filter.parse().unwrap_or(30);
        self.rule.min_size_mb = self.size_filter.parse().unwrap_or(100);
        self.apply_sort_and_filter();
        self.update_stats();
    }

    fn apply_sort_and_filter(&mut self) {
        let mut filtered = apply_rules(&self.all_files, &self.rule);
        
        // Apply file type filtering
        if self.filter_by_type != "All" {
            filtered.retain(|file| {
                let path = std::path::Path::new(&file.path);
                let ext = path.extension().unwrap_or_default().to_string_lossy().to_lowercase();
                match self.filter_by_type.as_str() {
                    "Images" => matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "gif" | "bmp" | "tiff" | "svg"),
                    "Documents" => matches!(ext.as_str(), "pdf" | "doc" | "docx" | "txt" | "rtf" | "odt" | "xls" | "xlsx" | "ppt" | "pptx"),
                    "Videos" => matches!(ext.as_str(), "mp4" | "avi" | "mov" | "wmv" | "flv" | "mkv" | "webm"),
                    _ => true,
                }
            });
        }
        
        // Apply sorting
        match self.sort_by {
            SortCriteria::Name => filtered.sort_by(|a, b| a.path.cmp(&b.path)),
            SortCriteria::Size => filtered.sort_by(|a, b| b.size.cmp(&a.size)),
            SortCriteria::Date => filtered.sort_by(|a, b| b.last_access_secs.cmp(&a.last_access_secs)),
            SortCriteria::Type => {
                filtered.sort_by(|a, b| {
                    let ext_a = std::path::Path::new(&a.path).extension().unwrap_or_default();
                    let ext_b = std::path::Path::new(&b.path).extension().unwrap_or_default();
                    ext_a.cmp(&ext_b)
                });
            }
        }
        
        self.files = filtered;
        self.selected = vec![false; self.files.len()];
        self.update_selection_stats();
    }

    fn update_stats(&mut self) {
        self.stats.total_files = self.all_files.len();
        self.stats.total_size = self.all_files.iter().map(|f| f.size).sum();
        
        if let Some(oldest) = self.all_files.iter().max_by_key(|f| f.last_access_secs) {
            self.stats.oldest_file = oldest.path.clone();
        }
        
        if let Some(newest) = self.all_files.iter().min_by_key(|f| f.last_access_secs) {
            self.stats.newest_file = newest.path.clone();
        }
        
        if let Some(largest) = self.all_files.iter().max_by_key(|f| f.size) {
            self.stats.largest_file = largest.path.clone();
        }
        
        // Count file types
        self.stats.file_types.clear();
        for file in &self.all_files {
            let path = std::path::Path::new(&file.path);
            let ext = path.extension().unwrap_or_default().to_string_lossy().to_lowercase();
            let file_type = if ext.is_empty() { "no_extension".to_string() } else { ext };
            *self.stats.file_types.entry(file_type).or_insert(0) += 1;
        }
    }

    fn update_selection_stats(&mut self) {
        self.selected_count = self.selected.iter().filter(|&&x| x).count();
        self.total_size_selected = self.selected.iter().enumerate()
            .filter(|(_, &selected)| selected)
            .map(|(i, _)| self.files.get(i).map(|f| f.size).unwrap_or(0))
            .sum();
    }
}

struct HeaderStyle;
struct EvenRowStyle;
struct OddRowStyle;

impl container::StyleSheet for HeaderStyle {
    type Style = Theme;

    fn appearance(&self, _style: &Self::Style) -> container::Appearance {
        container::Appearance {
            background: Some(iced::Background::Color(iced::Color::from_rgb8(200, 200, 200))),
            ..Default::default()
        }
    }
}

impl container::StyleSheet for EvenRowStyle {
    type Style = Theme;

    fn appearance(&self, _style: &Self::Style) -> container::Appearance {
        container::Appearance {
            background: Some(iced::Background::Color(iced::Color::from_rgb8(248, 248, 248))),
            ..Default::default()
        }
    }
}

impl container::StyleSheet for OddRowStyle {
    type Style = Theme;

    fn appearance(&self, _style: &Self::Style) -> container::Appearance {
        container::Appearance {
            background: Some(iced::Background::Color(iced::Color::from_rgb8(255, 255, 255))),
            ..Default::default()
        }
    }
}