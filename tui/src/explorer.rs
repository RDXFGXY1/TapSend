use std::{
    cmp::Ordering,
    collections::HashSet,
    fs, io,
    path::{Path, PathBuf},
};

#[derive(Clone)]
pub struct Entry {
    pub path: PathBuf,
    pub name: String,
    pub directory: bool,
}

pub struct Explorer {
    directory: PathBuf,
    entries: Vec<Entry>,
    cursor: usize,
    selected: HashSet<PathBuf>,
    show_hidden: bool,
}

impl Explorer {
    pub fn new(directory: PathBuf) -> io::Result<Self> {
        let mut explorer = Self {
            directory,
            entries: Vec::new(),
            cursor: 0,
            selected: HashSet::new(),
            show_hidden: false,
        };
        explorer.refresh()?;
        Ok(explorer)
    }

    pub fn set_show_hidden(&mut self, show: bool) -> io::Result<()> {
        if self.show_hidden != show {
            self.show_hidden = show;
            self.refresh()?;
        }
        Ok(())
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }
    pub fn cursor(&self) -> usize {
        self.cursor
    }
    pub fn selected(&self, path: &Path) -> bool {
        self.selected.contains(path)
    }
    pub fn selected_count(&self) -> usize {
        self.selected.len()
    }
    pub fn selected_files(&self) -> Vec<PathBuf> {
        let mut files: Vec<_> = self.selected.iter().cloned().collect();
        files.sort();
        files
    }
    pub fn selected_size(&self) -> u64 {
        self.selected
            .iter()
            .filter_map(|path| fs::metadata(path).ok())
            .map(|m| m.len())
            .sum()
    }

    pub fn up(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }
    pub fn down(&mut self) {
        if self.cursor + 1 < self.entries.len() {
            self.cursor += 1;
        }
    }
    pub fn parent(&mut self) -> io::Result<()> {
        if let Some(parent) = self.directory.parent() {
            self.directory = parent.to_path_buf();
            self.cursor = 0;
            self.refresh()?;
        }
        Ok(())
    }
    pub fn open(&mut self) -> io::Result<()> {
        if let Some(entry) = self.entries.get(self.cursor) {
            if entry.directory {
                self.directory = entry.path.clone();
                self.cursor = 0;
                self.refresh()?;
            }
        }
        Ok(())
    }
    pub fn toggle(&mut self) {
        if let Some(entry) = self.entries.get(self.cursor) {
            if !entry.directory && !self.selected.remove(&entry.path) {
                self.selected.insert(entry.path.clone());
            }
        }
    }
    pub fn clear_selection(&mut self) {
        self.selected.clear();
    }
    pub fn unselect(&mut self, path: &Path) {
        self.selected.remove(path);
    }
    pub fn focused_file(&self) -> Option<PathBuf> {
        self.entries
            .get(self.cursor)
            .filter(|entry| !entry.directory)
            .map(|entry| entry.path.clone())
    }
    pub fn toggle_hidden(&mut self) -> io::Result<()> {
        self.show_hidden = !self.show_hidden;
        self.refresh()
    }

    fn refresh(&mut self) -> io::Result<()> {
        let mut entries = Vec::new();
        for result in fs::read_dir(&self.directory)? {
            let item = match result {
                Ok(item) => item,
                Err(_) => continue,
            };
            let name = item.file_name().to_string_lossy().into_owned();
            if !self.show_hidden && name.starts_with('.') {
                continue;
            }
            let file_type = match item.file_type() {
                Ok(value) => value,
                Err(_) => continue,
            };
            if file_type.is_dir() || file_type.is_file() {
                entries.push(Entry {
                    path: item.path(),
                    name,
                    directory: file_type.is_dir(),
                });
            }
        }
        entries.sort_by(|a, b| match (a.directory, b.directory) {
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        });
        self.entries = entries;
        self.cursor = self.cursor.min(self.entries.len().saturating_sub(1));
        Ok(())
    }
}
