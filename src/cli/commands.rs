use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::lab::Lab;
use crate::model::{Content, SearchOpts, StatementKey, Synonyms};

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
