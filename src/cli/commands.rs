use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::lab::Lab;
use crate::model::{Content, SearchOpts, StatementKey, Synonyms};
use crate::service::ProjectManager;
use crate::config::load_local_config;

#[derive(Parser)]
#[command(name = "hypatia", about = "AI-oriented memory management system with code mining and hybrid search", version)]
pub struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Connect to a shelf directory
    Connect {
        /// Path to shelf directory
        path: PathBuf,
        /// Optional name for the shelf
        #[arg(short, long)]
        name: Option<String>,
    },
    /// Disconnect from a shelf
    Disconnect {
        name: String,
    },
    /// List connected shelves
    List,
    /// Execute a JSE query
    Query {
        /// JSE query as JSON string
        jse: String,
        /// Shelf to query
        #[arg(short, long, default_value = "default")]
        shelf: String,
    },
    /// Create a knowledge entry
    KnowledgeCreate {
        name: String,
        /// Content data
        #[arg(short, long, default_value = "")]
        data: String,
        /// Tags (comma-separated)
        #[arg(short, long, default_value = "")]
        tags: String,
        /// Synonyms (comma-separated)
        #[arg(short, long, default_value = "")]
        synonyms: String,
        /// Shelf name
        #[arg(short, long, default_value = "default")]
        shelf: String,
    },
    /// Get a knowledge entry
    KnowledgeGet {
        name: String,
        #[arg(short, long, default_value = "default")]
        shelf: String,
    },
    /// Delete a knowledge entry
    KnowledgeDelete {
        name: String,
        #[arg(short, long, default_value = "default")]
        shelf: String,
    },
    /// Create a statement (triple)
    StatementCreate {
        subject: String,
        predicate: String,
        object: String,
        /// Content data
        #[arg(short, long, default_value = "")]
        data: String,
        /// Synonyms as JSON
        #[arg(short, long)]
        synonyms: Option<String>,
        #[arg(short, long, default_value = "default")]
        shelf: String,
    },
    /// Delete a statement (triple)
    StatementDelete {
        subject: String,
        predicate: String,
        object: String,
        #[arg(short, long, default_value = "default")]
        shelf: String,
    },
    /// Search knowledge and statements (FTS)
    Search {
        query: String,
        #[arg(short, long)]
        catalog: Option<String>,
        #[arg(long, default_value_t = 100)]
        limit: i64,
        #[arg(long, default_value_t = 0)]
        offset: i64,
        #[arg(short, long, default_value = "default")]
        shelf: String,
    },
    /// Vector similarity search (requires embedding model)
    Vsearch {
        query: String,
        /// Shelf name
        #[arg(short, long, default_value = "default")]
        shelf: String,
        #[arg(long, default_value_t = 10)]
        limit: i64,
    },
    /// Hybrid search (FTS + vector with RRF fusion)
    Hybrid {
        query: String,
        /// Shelf name
        #[arg(short, long, default_value = "default")]
        shelf: String,
        #[arg(long, default_value_t = 10)]
        limit: i64,
    },
    /// Initialize a new Hypatia library
    Init {
        /// Library directory
        #[arg(default_value = "~/.hypatia")]
        path: PathBuf,
    },
    /// Index a directory (mine code files)
    Mine {
        /// Directory to index
        path: PathBuf,
        /// Shelf name to store indexed chunks
        #[arg(short, long, default_value = "default")]
        shelf: String,
        /// Maximum file size (bytes)
        #[arg(long, default_value_t = 1048576)]
        max_size: usize,
        /// Chunk size (chars)
        #[arg(long, default_value_t = 512)]
        chunk_size: usize,
        /// Include hidden directories
        #[arg(long)]
        hidden: bool,
    },
    /// Incremental re-index (only changed files)
    Watch {
        /// Directory to scan
        path: PathBuf,
        #[arg(short, long, default_value = "default")]
        shelf: String,
    },
    /// Show library status (vector count, FTS docs, shelves)
    Status {
        #[arg(short, long, default_value = "default")]
        shelf: String,
    },
    /// Health check and diagnostics
    Doctor {
        #[arg(short, long, default_value = "default")]
        shelf: String,
    },
    /// Export a shelf to another directory
    Export {
        name: String,
        dest: PathBuf,
    },
    /// Enter interactive REPL mode
    Repl,
    /// Project management commands
    Project {
        #[command(subcommand)]
        action: ProjectAction,
    },
    /// Initialize a project in the current directory
    ProjectInit {
        /// Project name (defaults to directory name)
        #[arg(short, long)]
        name: Option<String>,
        /// Wing to assign this project to
        #[arg(short, long)]
        wing: Option<String>,
        /// Room to assign this project to
        #[arg(long)]
        room: Option<String>,
        /// Shelf to use (defaults to project name)
        #[arg(short, long)]
        shelf: Option<String>,
    },
    /// Mine a registered project by name
    ProjectMine {
        /// Project name
        name: String,
    },
    /// Start background watch daemon
    DaemonStart,
    /// Stop watch daemon
    DaemonStop,
    /// Show daemon status
    DaemonStatus,
    /// Force rescan of all watched directories
    DaemonRescan,
    /// Rebuild FTS index from docs_meta (fix count mismatch)
    RebuildFts {
        #[arg(short, long, default_value = "default")]
        shelf: String,
    },
}

#[derive(Subcommand)]
enum ProjectAction {
    /// Register a new project
    Add {
        /// Project name
        name: String,
        /// Root directory of the project
        #[arg(short, long, default_value = ".")]
        root: PathBuf,
        /// Shelf name (defaults to project name)
        #[arg(short, long)]
        shelf: Option<String>,
        /// Wing assignment
        #[arg(short, long)]
        wing: Option<String>,
        /// Room assignment
        #[arg(long)]
        room: Option<String>,
    },
    /// Remove a project from the registry
    Remove {
        name: String,
    },
    /// List all registered projects
    List,
    /// Show project details
    Show {
        name: String,
    },
    /// Toggle auto-watch for a project
    AutoWatch {
        name: String,
        /// Enable or disable auto-watch
        #[arg(long)]
        enable: bool,
        /// Disable auto-watch
        #[arg(long)]
        disable: bool,
    },
    /// Mine a specific project by name
    Mine {
        /// Project name
        name: String,
    },
}

pub fn run() -> crate::error::Result<()> {
    let cli = Cli::parse();
    let mut lab = Lab::new()?;

    match cli.command {
        None | Some(Commands::Repl) => {
            let mut repl = super::repl::Repl::new(lab)?;
            repl.run()
        }
        Some(cmd) => execute_command(&mut lab, cmd),
    }
}

fn execute_command(lab: &mut Lab, cmd: Commands) -> crate::error::Result<()> {
    match cmd {
        Commands::Connect { path, name } => {
            let shelf_name = lab.connect_shelf(&path, name.as_deref())?;
            println!("Connected to shelf: {shelf_name}");
        }
        Commands::Disconnect { name } => {
            lab.disconnect_shelf(&name)?;
            println!("Disconnected from shelf: {name}");
        }
        Commands::List => {
            let shelves = lab.list_shelves();
            if shelves.is_empty() {
                println!("No shelves connected.");
            } else {
                for name in &shelves {
                    println!("  {name}");
                }
            }
        }
        Commands::Query { jse, shelf } => {
            let value: serde_json::Value = serde_json::from_str(&jse)?;
            let result = lab.query(&shelf, &value)?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Commands::KnowledgeCreate {
            name,
            data,
            tags,
            synonyms,
            shelf,
        } => {
            let tag_vec: Vec<String> = if tags.is_empty() {
                vec![]
            } else {
                tags.split(',').map(|s| s.trim().to_string()).collect()
            };
            let syn_vec: Vec<String> = if synonyms.is_empty() {
                vec![]
            } else {
                synonyms.split(',').map(|s| s.trim().to_string()).collect()
            };
            let content = Content::new(data)
                .with_tags(tag_vec)
                .with_synonyms(if syn_vec.is_empty() {
                    None
                } else {
                    Some(Synonyms::Flat(syn_vec))
                });
            let knowledge = lab.create_knowledge(&shelf, &name, content)?;
            println!("Created knowledge: {}", knowledge.name);
        }
        Commands::KnowledgeGet { name, shelf } => {
            match lab.get_knowledge(&shelf, &name)? {
                Some(k) => println!("{}", serde_json::to_string_pretty(&k)?),
                None => println!("Knowledge '{}' not found", name),
            }
        }
        Commands::KnowledgeDelete { name, shelf } => {
            lab.delete_knowledge(&shelf, &name)?;
            println!("Deleted knowledge: {}", name);
        }
        Commands::StatementDelete {
            subject,
            predicate,
            object,
            shelf,
        } => {
            let key = StatementKey::new(subject, predicate, object);
            lab.delete_statement(&shelf, &key)?;
            println!("Deleted statement: {}", key.to_csv_key());
        }
        Commands::StatementCreate {
            subject,
            predicate,
            object,
            data,
            synonyms,
            shelf,
        } => {
            let key = StatementKey::new(subject, predicate, object);
            let syn = if let Some(s) = synonyms {
                let map: std::collections::HashMap<String, Vec<String>> =
                    serde_json::from_str(&s)?;
                Some(Synonyms::Positional(map))
            } else {
                None
            };
            let content = Content::new(data).with_synonyms(syn);
            let stmt = lab.create_statement(&shelf, &key, content, None, None)?;
            println!(
                "Created statement: {} {} {}",
                stmt.key.subject, stmt.key.predicate, stmt.key.object
            );
        }
        Commands::Search {
            query,
            catalog,
            limit,
            offset,
            shelf,
        } => {
            let opts = SearchOpts {
                catalog,
                limit,
                offset,
            };
            let result = lab.search(&shelf, &query, &opts)?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Commands::Vsearch { query, shelf, limit } => {
            let result = lab.vector_search(&shelf, &query, limit)?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Commands::Hybrid { query, shelf, limit } => {
            let result = lab.hybrid_search(&shelf, &query, limit)?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Commands::Init { path } => {
            let expanded = shellexpand_tilde(&path);
            // Download embedding model if not present
            super::init_model::ensure_models()?;
            lab.init_library(&expanded)?;
            println!("Initialized Hypatia library at: {}", expanded.display());
        }
        Commands::Mine {
            path,
            shelf,
            max_size,
            chunk_size,
            hidden,
        } => {
            let count = lab.mine_directory(&shelf, &path, max_size, chunk_size, hidden)?;
            println!("Indexed {} chunks from {}", count, path.display());
        }
        Commands::Watch { path, shelf } => {
            let (new, modified, deleted) = lab.incremental_scan(&shelf, &path)?;
            println!("Watch results: +{} new, ~{} modified, -{} deleted", new, modified, deleted);
        }
        Commands::Status { shelf } => {
            let status = lab.get_status(&shelf)?;
            println!("{}", status);
        }
        Commands::Doctor { shelf } => {
            let report = lab.run_doctor(&shelf)?;
            println!("{}", report);
        }
        Commands::Export { name, dest } => {
            lab.export_shelf(&name, &dest)?;
            println!("Exported shelf '{}' to '{}'", name, dest.display());
        }
        Commands::Repl => {
            // Repl is handled in run(), this should not be reached
            unreachable!("Repl command should be handled in run()")
        }
        Commands::Project { action } => {
            handle_project_action(action)?;
        }
        Commands::ProjectInit { name, wing, room, shelf } => {
            let current_dir = std::env::current_dir()?;
            let project_name = name.unwrap_or_else(|| {
                current_dir.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("default")
                    .to_string()
            });
            let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
            let hypatia_home = home.join(".hypatia");
            let mut manager = ProjectManager::new(&hypatia_home)?;
            
            let project = manager.add_project(
                project_name.clone(),
                current_dir.clone(),
                shelf.or(Some(project_name.clone())),
                wing,
                room,
            )?;
            
            println!("Initialized project '{}' at {}", project.name, project.root.display());
            println!("Shelf: {}", project.shelf);
            if let Some(w) = &project.wing {
                println!("Wing: {}", w);
            }
            if let Some(r) = &project.room {
                println!("Room: {}", r);
            }
            println!("Config file: {}/.hypatia/project.toml", project.root.display());
        }
        Commands::ProjectMine { name } => {
            let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
            let hypatia_home = home.join(".hypatia");
            let manager = ProjectManager::new(&hypatia_home)?;
            
            match manager.get_project(&name) {
                Some(project) => {
                    let local_config = load_local_config(&project.root)?;
                    let (max_size, chunk_size, _skip_patterns, _extensions) = if let Some(cfg) = local_config {
                        (cfg.max_file_size, cfg.chunk_size, cfg.skip_patterns, cfg.extensions)
                    } else {
                        (project.max_file_size, project.chunk_size, project.skip_patterns.clone(), project.extensions.clone())
                    };
                    
                    let count = lab.mine_directory(
                        &project.shelf,
                        &project.root,
                        max_size,
                        chunk_size,
                        false,
                    )?;
                    println!("Indexed {} chunks from project '{}'", count, name);
                }
                None => {
                    println!("Project '{}' not found. Use 'project list' to see registered projects.", name);
                }
            }
        }
        Commands::DaemonStart => {
            use crate::daemon::WatchDaemon;
            let mut daemon = WatchDaemon::new()?;
            daemon.start()?;
            println!("Daemon running. Use hypatia daemon-status to check.");
            std::thread::park();
        }
        Commands::DaemonStop => {
            use crate::daemon::WatchDaemon;
            let mut daemon = WatchDaemon::new()?;
            daemon.stop()?;
        }
        Commands::DaemonStatus => {
            use crate::daemon::WatchDaemon;
            let daemon = WatchDaemon::new()?;
            daemon.status()?;
        }
        Commands::DaemonRescan => {
            use crate::daemon::WatchDaemon;
            let mut daemon = WatchDaemon::new()?;
            daemon.rescan()?;
        }
        Commands::RebuildFts { shelf } => {
            let (meta_count, fts_count) = lab.rebuild_fts(&shelf)?;
            println!("Rebuilt FTS index: {} docs_meta -> {} docs_fts entries", meta_count, fts_count);
        }
    }
    Ok(())
}

fn shellexpand_tilde(path: &std::path::Path) -> std::path::PathBuf {
    if let Some(s) = path.to_str() {
        if s.starts_with("~/") {
            if let Some(home) = dirs::home_dir() {
                return home.join(&s[2..]);
            }
        }
    }
    path.to_path_buf()
}
fn handle_project_action(action: ProjectAction) -> crate::error::Result<()> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let hypatia_home = home.join(".hypatia");
    let mut manager = ProjectManager::new(&hypatia_home)?;

    match action {
        ProjectAction::Add { name, root, shelf, wing, room } => {
            let canonical_root = root.canonicalize()
                .map_err(|e| crate::error::HypatiaError::IoMsg(
                    format!("Directory does not exist: {}: {}", root.display(), e)
                ))?;
            let project = manager.add_project(
                name,
                canonical_root,
                shelf,
                wing,
                room,
            )?;
            println!("Added project '{}' -> {}", project.name, project.root.display());
            println!("  Shelf: {}", project.shelf);
            if let Some(w) = &project.wing { println!("  Wing: {}", w); }
            if let Some(r) = &project.room { println!("  Room: {}", r); }
        }
        ProjectAction::Remove { name } => {
            match manager.remove_project(&name)? {
                Some(p) => println!("Removed project '{}' ({})", p.name, p.root.display()),
                None => println!("Project '{}' not found", name),
            }
        }
        ProjectAction::List => {
            let projects = manager.list_projects();
            if projects.is_empty() {
                println!("No projects registered. Use 'project add <name>' to register one.");
            } else {
                println!("Registered projects:");
                println!("{:<20} {:<15} {:<10} {:<10} {}", "Name", "Shelf", "Wing", "Room", "Path");
                println!("{}", "-".repeat(90));
                for p in projects {
                    println!(
                        "{:<20} {:<15} {:<10} {:<10} {}",
                        p.name,
                        p.shelf,
                        p.wing.as_deref().unwrap_or("-"),
                        p.room.as_deref().unwrap_or("-"),
                        p.root.display(),
                    );
                }
            }
        }
        ProjectAction::Show { name } => {
            match manager.get_project(&name) {
                Some(p) => {
                    println!("Project: {}", p.name);
                    println!("  Root: {}", p.root.display());
                    println!("  Shelf: {}", p.shelf);
                    if let Some(w) = &p.wing { println!("  Wing: {}", w); }
                    if let Some(r) = &p.room { println!("  Room: {}", r); }
                    println!("  Auto-watch: {}", if p.auto_watch { "enabled" } else { "disabled" });
                    println!("  Max file size: {} bytes", p.max_file_size);
                    println!("  Chunk size: {} chars", p.chunk_size);
                    println!("  Extensions: {}", p.extensions.join(", "));
                    println!("  Skip patterns: {}", p.skip_patterns.join(", "));
                    println!("  Created: {}", p.created_at);
                    if let Some(t) = &p.last_indexed {
                        println!("  Last indexed: {}", t);
                    }
                }
                None => println!("Project '{}' not found", name),
            }
        }
        ProjectAction::AutoWatch { name, enable, disable } => {
            if enable && disable {
                println!("Cannot use both --enable and --disable");
            } else if enable {
                manager.toggle_auto_watch(&name, true)?;
                println!("Auto-watch enabled for project '{}'", name);
            } else if disable {
                manager.toggle_auto_watch(&name, false)?;
                println!("Auto-watch disabled for project '{}'", name);
            } else {
                println!("Use --enable or --disable to toggle auto-watch");
            }
        }
        ProjectAction::Mine { name } => {
            match manager.get_project(&name) {
                Some(project) => {
                    let mut lab = Lab::new()?;
                    let count = lab.mine_directory(
                        &project.shelf,
                        &project.root,
                        project.max_file_size,
                        project.chunk_size,
                        false,
                    )?;
                    println!("Indexed {} chunks from project '{}'", count, name);
                }
                None => {
                    println!("Project '{}' not found", name);
                }
            }
        }
    }
    Ok(())
}
