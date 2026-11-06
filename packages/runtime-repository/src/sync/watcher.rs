use std::path::PathBuf;
use std::sync::{mpsc, Mutex};
use std::time::SystemTime;
use notify::{Watcher, RecursiveMode, Event, EventKind};
use crate::sync::change::{RepositoryChange, ChangeSource, ChangeType};
use shared::types::Result;

pub trait RepositoryWatcher: Send + Sync {
    fn start(&mut self) -> Result<()>;
    fn stop(&mut self) -> Result<()>;
}

pub struct NotifyWatcher {
    path: PathBuf,
    watcher: Option<notify::RecommendedWatcher>,
    rx: Mutex<Option<mpsc::Receiver<RepositoryChange>>>,
}

impl NotifyWatcher {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            watcher: None,
            rx: Mutex::new(None),
        }
    }

    pub fn subscribe(&mut self) -> Option<mpsc::Receiver<RepositoryChange>> {
        if let Ok(mut guard) = self.rx.lock() {
            guard.take()
        } else {
            None
        }
    }
}

impl RepositoryWatcher for NotifyWatcher {
    fn start(&mut self) -> Result<()> {
        let (tx, rx) = mpsc::channel();
        if let Ok(mut guard) = self.rx.lock() {
            *guard = Some(rx);
        }

        let mut watcher = notify::recommended_watcher(move |res: std::result::Result<Event, notify::Error>| {
            if let Ok(event) = res {
                for path in event.paths {
                    let change_type = match event.kind {
                        EventKind::Create(_) => ChangeType::FileCreated,
                        EventKind::Modify(_) => ChangeType::FileModified,
                        EventKind::Remove(_) => ChangeType::FileDeleted,
                        _ => continue,
                    };
                    let change = RepositoryChange {
                        source: ChangeSource::Watcher,
                        timestamp: SystemTime::now(),
                        change_type,
                        affected_path: path,
                        metadata: None,
                    };
                    let _ = tx.send(change);
                }
            }
        }).map_err(|e| shared::error::AtlasError::Repository(shared::error::RepositoryError::FileNotFound(e.to_string())))?;

        watcher.watch(&self.path, RecursiveMode::Recursive)
            .map_err(|e| shared::error::AtlasError::Repository(shared::error::RepositoryError::FileNotFound(e.to_string())))?;

        self.watcher = Some(watcher);
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        if let Some(mut watcher) = self.watcher.take() {
            let _ = watcher.unwatch(&self.path);
        }
        Ok(())
    }
}
