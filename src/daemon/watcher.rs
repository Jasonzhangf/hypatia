use crate::daemon::state::DaemonState;
use crate::error::{HypatiaError, Result};
use crate::lab::Lab;
use crate::service::ProjectManager;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};
use std::thread;
use std::time::Duration;

/// Watch daemon that monitors multiple directories
pub struct WatchDaemon {
    state: DaemonState,
    watcher: Option<RecommendedWatcher>,
    events_rx: Option<Receiver<std::result::Result<Event, notify::Error>>>,
}

impl WatchDaemon {
    /// Create a new watch daemon
    pub fn new() -> Result<Self> {
        let state = DaemonState::load()?;
        Ok(Self {
            state,
            watcher: None,
            events_rx: None,
        })
    }

    /// Start the daemon - monitors all registered projects with auto_watch enabled
    pub fn start(&mut self) -> Result<()> {
        if self.state.is_running && self.state.is_process_alive() {
            println!("Daemon already running (PID: {})", self.state.pid.unwrap_or(0));
            return Ok(());
        }

        // Load project registry
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let hypatia_home = home.join(".hypatia");
        let manager = ProjectManager::new(&hypatia_home)?;

        // Find all projects with auto_watch enabled
        let auto_watch_projects: Vec<_> = manager.list_projects()
            .iter()
            .filter(|p| p.auto_watch)
            .collect();

        if auto_watch_projects.is_empty() {
            println!("No projects with auto-watch enabled. Use 'project auto-watch <name> --enable' first.");
            return Ok(());
        }

        println!("Starting daemon, watching {} projects:", auto_watch_projects.len());
        for p in &auto_watch_projects {
            println!("  - {} ({})", p.name, p.root.display());
        }

        // Setup watcher
        let (tx, rx) = channel();
        let mut watcher = RecommendedWatcher::new(
            move |res| {
                let _ = tx.send(res);
            },
            Config::default(),
        ).map_err(|e| HypatiaError::IoMsg(e.to_string()))?;

        // Watch each project directory
        for p in &auto_watch_projects {
            watcher.watch(&p.root, RecursiveMode::Recursive)
                .map_err(|e| HypatiaError::IoMsg(e.to_string()))?;
            self.state.add_watch(
                p.root.to_string_lossy().as_ref(),
                &p.shelf,
                &p.name,
            );
        }

        self.watcher = Some(watcher);
        self.events_rx = Some(rx);

        // Mark as running
        let pid = std::process::id();
        self.state.mark_running(pid);
        self.state.save()?;

        println!("Daemon started (PID: {}). Monitoring for file changes...", pid);

        // Start event processing loop in background thread
        self.run_event_loop();

        Ok(())
    }

    /// Run the event processing loop
    fn run_event_loop(&mut self) {
        let rx = self.events_rx.take().expect("Events receiver not set");
        let hypatia_home = dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")).join(".hypatia");

        thread::spawn(move || {
            let debounce_time = Duration::from_secs(2);
            let mut pending_changes: HashMap<PathBuf, String> = HashMap::new();
            let mut last_process_time = std::time::Instant::now();

            loop {
                match rx.recv_timeout(Duration::from_millis(100)) {
                    Ok(Ok(event)) => {
                        // Filter relevant file events
                        if is_relevant_event(&event) {
                            for path in &event.paths {
                                let shelf = determine_shelf_for_path(path, &hypatia_home);
                                if let Some(s) = shelf {
                                    pending_changes.insert(path.clone(), s);
                                }
                            }
                        }
                    }
                    Ok(Err(e)) => {
                        eprintln!("Watch error: {}", e);
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                        if !pending_changes.is_empty() && last_process_time.elapsed() > debounce_time {
                            process_pending_changes(&pending_changes, &hypatia_home);
                            pending_changes.clear();
                            last_process_time = std::time::Instant::now();

                            if let Ok(state) = DaemonState::load() {
                                let mut state = state;
                                state.last_check = Some(chrono::Utc::now().to_rfc3339());
                                let _ = state.save();
                            }
                        }
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                        println!("Watcher disconnected, stopping daemon");
                        break;
                    }
                }
            }

            if let Ok(state) = DaemonState::load() {
                let mut state = state;
                state.mark_stopped();
                let _ = state.save();
            }
        });
    }

    /// Stop the daemon
    pub fn stop(&mut self) -> Result<()> {
        if !self.state.is_running {
            println!("Daemon is not running");
            return Ok(());
        }

        if self.state.pid == Some(std::process::id()) {
            self.watcher = None;
            self.events_rx = None;
        }

        self.state.mark_stopped();
        self.state.save()?;
        println!("Daemon stopped");
        Ok(())
    }

    /// Show daemon status
    pub fn status(&self) -> Result<()> {
        println!("Daemon Status:");
        println!("  Running: {}", self.state.is_running);
        if let Some(pid) = self.state.pid {
            println!("  PID: {}", pid);
            println!("  Process alive: {}", self.state.is_process_alive());
        }
        if let Some(t) = &self.state.last_check {
            println!("  Last check: {}", t);
        }
        println!("\nWatched directories:");
        if self.state.watched.is_empty() {
            println!("  (none)");
        } else {
            for entry in &self.state.watched {
                println!("  - {} [shelf: {}, project: {}]", entry.path, entry.shelf, entry.project_name);
                if let Some(t) = &entry.last_modified {
                    println!("    Last modified: {}, files indexed: {}", t, entry.files_indexed);
                }
            }
        }
        Ok(())
    }

    /// Force a rescan of all watched directories
    pub fn rescan(&mut self) -> Result<()> {
        let mut lab = Lab::new()?;
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let hypatia_home = home.join(".hypatia");
        let manager = ProjectManager::new(&hypatia_home)?;

        // Clone watched list to avoid borrow conflicts
        let entries: Vec<_> = self.state.watched.iter().map(|e| e.clone()).collect();

        for entry in entries {
            lab.ensure_shelf(&entry.shelf)?;
            let path = PathBuf::from(&entry.path);
            if let Some(project) = manager.get_project(&entry.project_name) {
                let count = lab.mine_directory(&entry.shelf.clone(), &path, project.max_file_size, project.chunk_size, false)?;
                println!("Rescanned {} -> {} chunks", entry.project_name, count);

                // Update entry using path as key
                if let Some(found) = self.state.watched.iter_mut().find(|e| e.path == entry.path) {
                    found.files_indexed = count;
                    found.last_modified = Some(chrono::Utc::now().to_rfc3339());
                }
            }
        }
        self.state.save()?;
        Ok(())
    }
}

/// Check if an event is relevant for indexing
fn is_relevant_event(event: &Event) -> bool {
    matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_))
        && event.paths.iter().any(|p| {
            let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
            matches!(ext, "rs" | "ts" | "js" | "py" | "md" | "json" | "yaml" | "yml" | "toml" | "txt")
        })
}

/// Determine which shelf a path belongs to
fn determine_shelf_for_path(path: &PathBuf, hypatia_home: &PathBuf) -> Option<String> {
    if let Ok(manager) = ProjectManager::new(hypatia_home) {
        for project in manager.list_projects() {
            if path.starts_with(&project.root) {
                return Some(project.shelf.clone());
            }
        }
    }
    None
}

/// Process pending file changes
fn process_pending_changes(changes: &HashMap<PathBuf, String>, hypatia_home: &PathBuf) {
    let mut lab = match Lab::new() {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Failed to create Lab: {}", e);
            return;
        }
    };

    // Group changes by shelf
    let by_shelf: HashMap<String, Vec<PathBuf>> = changes.iter()
        .fold(HashMap::new(), |mut acc, (path, shelf)| {
            acc.entry(shelf.clone()).or_insert_with(Vec::new).push(path.clone());
            acc
        });

    for (shelf, paths) in by_shelf {
        if let Err(e) = lab.ensure_shelf(&shelf) {
            eprintln!("Failed to ensure shelf '{}': {}", shelf, e);
            continue;
        }
        println!("Processing {} changed files for shelf '{}'", paths.len(), shelf);

        if let Ok(manager) = ProjectManager::new(hypatia_home) {
            if let Some(project) = manager.list_projects().iter().find(|p| p.shelf == shelf) {
                let root = project.root.clone();
                let max_size = project.max_file_size;
                let chunk_size = project.chunk_size;
                let _ = lab.mine_directory(&shelf, &root, max_size, chunk_size, true);
            }
        }
    }
}
