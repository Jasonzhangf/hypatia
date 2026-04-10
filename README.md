# Hypatia

"We can wander through the stacks of the Library of Alexandria, imagining the scrolls and the knowledge they contain. Its destruction is a warning: all we have is transient."——Alberto Manguel

AI-oriented memory management system with **hybrid search** (FTS + semantic), code mining, and project management. Built on SQLite FTS5 + DuckDB, with Candle embeddings for vector search.

## Features

- **Knowledge Graph** -- Knowledge entries (named info points with tags) and Statement triples (subject-predicate-object)
- **JSE Query Engine** -- JSON-based query language compiling to parameterized SQL + FTS5
- **Hybrid Search** -- FTS for exact keywords, vector for semantic/fuzzy, RRF fusion for best results
- **Project Management** -- Register and track multiple code projects with wing/room hierarchy
- **Code Mining** -- Index source files into searchable chunks with configurable skip patterns
- **Config Files** -- Per-project `.hypatia/project.toml` with extensions, skip patterns, and settings
- **Shelf System** -- Named, connectable, exportable data directories for isolation
- **CLI + REPL** -- Full command-line interface with interactive mode
- **Cross-Platform** -- Build for 18+ targets

## Installation

```bash
git clone https://github.com/Jasonzhangf/hypatia.git
cd hypatia
cargo build --release
cp target/release/hypatia ~/.local/bin/
```

## Quick Start

```bash
# Initialize library (downloads embedding model ~86MB)
hypatia init

# Register a project
cd ~/github/myproject
hypatia project-init --wing code

# Mine (index) the project
hypatia project mine myproject

# Search
hypatia hybrid "error handling" --shelf myproject
```

## Search Types

| Command | Use Case | Example |
|---------|----------|---------|
| `search` (FTS) | Exact keywords, code symbols | `hypatia search "fn main"` |
| `vsearch` (Vector) | Fuzzy matching, typos, semantics | `hypatia vsearch "err handlig"` |
| `hybrid` | Best of both (recommended) | `hypatia hybrid "memory system"` |

Use `search` for code symbols and exact matches. Use `vsearch` or `hybrid` for user input that may have typos or vague queries.

## Project Management

```bash
# Initialize project in current directory
hypatia project-init --name myproject --wing code --room rust

# Or manually register
hypatia project add myproject --root ~/github/myproject --wing work

# List all projects
hypatia project list

# Show project details
hypatia project show myproject

# Mine a registered project
hypatia project mine myproject

# Remove a project
hypatia project remove myproject
```

### Project Config (.hypatia/project.toml)

```toml
name = "myproject"
wing = "code"
room = "rust"

skip_patterns = ["target/**", "node_modules/**", "*.lock"]
extensions = ["rs", "ts", "md", "json"]
max_file_size = 1048576
chunk_size = 512
```

## Mining Commands

```bash
# Mine a directory directly
hypatia mine ~/github/myproject --shelf myproject

# Mine registered project (uses config)
hypatia project mine myproject

# Watch for changes (incremental)
hypatia watch ~/github/myproject --shelf myproject
```

## CLI Reference

| Command | Description |
|---------|-------------|
| `hypatia init [path]` | Initialize library, download model |
| `hypatia project-init` | Initialize project in current directory |
| `hypatia project add <name>` | Register a project |
| `hypatia project list` | List all projects |
| `hypatia project show <name>` | Show project details |
| `hypatia project mine <name>` | Mine a registered project |
| `hypatia project remove <name>` | Remove project from registry |
| `hypatia mine <path>` | Index a directory |
| `hypatia watch <path>` | Watch for changes |
| `hypatia search <query>` | FTS exact search |
| `hypatia vsearch <query>` | Vector semantic search |
| `hypatia hybrid <query>` | Hybrid search (FTS + vector) |
| `hypatia knowledge-create <name>` | Create knowledge entry |
| `hypatia statement-create <s> <p> <o>` | Create triple |
| `hypatia query '<jse>'` | JSE structured query |
| `hypatia repl` | Interactive REPL |

## JSE Query Language

```json
["$knowledge", ["$eq", "name", "Rust"]]
["$statement", ["$triple", "Rust", "$*", "$*"]]
["$knowledge", ["$search", "database migration"]]
```

See CLI Reference for full operator list.

## Wing/Room Hierarchy

Hypatia supports mempalace-style organization:
- **Shelf**: Top-level storage unit (defaults to project name)
- **Wing**: Category within shelf (optional)
- **Room**: Sub-category within wing (optional)

```bash
hypatia project add client-api --wing work --room production
hypatia project add personal-blog --wing personal --room blog
```

## Comparison with Mempalace

| Feature | Hypatia | Mempalace |
|---------|---------|-----------|
| FTS | SQLite FTS5 | Not native |
| Vector search | Candle embeddings | Candle embeddings |
| Hybrid search | RRF fusion | Not native |
| Wing/Room | Supported | Core concept |
| Project registry | Yes | Yes |
| Auto-watch daemon | Planned | Yes |
| Config files | project.toml | .mempalace_ignore |

## Architecture

- **SQLite**: FTS storage with Porter stemmer + BM25
- **DuckDB**: Vector storage (384-dim embeddings)
- **Candle**: Local embedding inference (all-MiniLM-L6-v2)
- **ignore crate**: Gitignore-style skip patterns for mining

## License

MIT

## See Also

- `skills/hypatia-usage/SKILL.md` -- Detailed usage guide with troubleshooting
