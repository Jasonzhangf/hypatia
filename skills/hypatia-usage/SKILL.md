# Hypatia CLI Usage Guide

## Overview
Hypatia is an AI-oriented memory management system with:
- FTS (Full-Text Search) for exact keyword matching
- Vector/semantic search for fuzzy matching
- Hybrid search combining both with RRF fusion
- Code mining for indexing source files
- Project management with wing/room hierarchy

## Initialization

### First-time setup
```bash
# Initialize Hypatia library (downloads embedding model)
hypatia init ~/.hypatia

# Or with default path
hypatia init
```

This downloads the `all-MiniLM-L6-v2` embedding model (~86MB) to `~/.hypatia/models/`.

### Project initialization
```bash
# In your project directory
cd ~/github/myproject
hypatia project-init

# Or specify options
hypatia project-init --name myproject --wing code --room rust
```

Creates:
- `~/.hypatia/projects.json` - project registry
- `.hypatia/project.toml` - local project config

## Project Management

### Register projects
```bash
# Add a project
hypatia project add myproject --root ~/github/myproject --wing work

# List all projects
hypatia project list

# Show project details
hypatia project show myproject

# Remove a project
hypatia project remove myproject

# Enable auto-watch (future feature)
hypatia project auto-watch myproject --enable
```

### Project config file (.hypatia/project.toml)
```toml
name = "myproject"
wing = "code"
room = "rust"

# Skip patterns (extends defaults)
skip_patterns = [
    "target/**",
    "node_modules/**",
    "*.lock",
]

# File extensions to include
extensions = ["rs", "ts", "md"]

# Mining settings
max_file_size = 1048576  # 1MB
chunk_size = 512
```

## Mining (Indexing)

### Mine a directory directly
```bash
# Index a directory into a shelf
hypatia mine ~/github/myproject --shelf myproject

# With options
hypatia mine ~/github/myproject --shelf myproject --max-size 2097152 --chunk-size 1024
```

### Mine a registered project
```bash
# Uses project's config settings
hypatia project mine myproject
```

### Incremental watch (scan changes)
```bash
hypatia watch ~/github/myproject --shelf myproject
```

## Search Types

### When to use each search type

| Search Type | Use Case | Example |
|-------------|----------|---------|
| `search` (FTS) | Exact keywords, code symbols, specific terms | `hypatia search "fn main"` |
| `vsearch` | Fuzzy matching, semantic search, typos, related concepts | `hypatia vsearch "error handling"` |
| `hybrid` | Best of both, general queries | `hypatia hybrid "memory system"` |

### Exact search (FTS)
```bash
# Search for exact keywords
hypatia search "struct Knowledge" --shelf myproject

# With catalog filter
hypatia search "mod config" --catalog knowledge --shelf myproject

# Pagination
hypatia search "function" --limit 50 --offset 0 --shelf myproject
```

### Semantic search (Vector)
```bash
# Search by meaning (handles typos, synonyms)
hypatia vsearch "error handling logic" --shelf myproject --limit 10

# "err handlig" still finds error handling content
hypatia vsearch "err handlig" --shelf myproject
```

### Hybrid search (Recommended)
```bash
# Combines FTS + vector with RRF fusion
hypatia hybrid "memory management" --shelf myproject --limit 20
```

## Wing/Room Hierarchy

Hypatia supports mempalace-style organization:
- **Shelf**: Top-level storage unit (default: project name)
- **Wing**: Category within shelf (optional)
- **Room**: Sub-category within wing (optional)

```bash
# Register project with hierarchy
hypatia project add client-api --wing work --room production
hypatia project add personal-blog --wing personal --room blog

# List shows hierarchy
hypatia project list
```

## Knowledge & Statements

### Knowledge entries (named content)
```bash
# Create
hypatia knowledge-create "API Design" --data "REST endpoints for..." --tags api,design --shelf myproject

# Get
hypatia knowledge-get "API Design" --shelf myproject

# Delete
hypatia knowledge-delete "API Design" --shelf myproject
```

### Statements (triples for relationships)
```bash
# Create triple
hypatia statement-create "Hypatia" "uses" "DuckDB" --data "for FTS storage" --shelf myproject

# Delete triple
hypatia statement-delete "Hypatia" "uses" "DuckDB" --shelf myproject
```

## Multi-Project Workflow

### Typical setup
```bash
# 1. Initialize
hypatia init

# 2. Register all your code projects
hypatia project add routecodex --wing code --room rust
hypatia project add dify --wing code --room python
hypatia project add obsidian --wing personal --room notes

# 3. Mine each project
hypatia project mine routecodex
hypatia project mine dify
hypatia project mine obsidian

# 4. Search across shelves
hypatia hybrid "error handling" --shelf routecodex
hypatia hybrid "agent workflow" --shelf dify
```

### Cross-project search
Each project has its own shelf. Search per shelf or connect multiple shelves.

```bash
# Connect shelves
hypatia connect ~/.hypatia/shelves/routecodex --name routecodex
hypatia connect ~/.hypatia/shelves/dify --name dify

# List connected shelves
hypatia list
```

## Best Practices

1. **Use `project-init` for new projects** - auto-generates config file
2. **Edit `.hypatia/project.toml`** to customize skip patterns
3. **Use `hybrid` search** as default for best results
4. **Use `search` (FTS)** for code symbols and exact matches
5. **Use `vsearch`** for user input that may have typos
6. **Group projects by wing/room** for organization
7. **Re-mine periodically** or use watch for incremental updates

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

Hypatia provides both exact and semantic search, making it suitable for:
- **Code search**: FTS for symbols, vector for concepts
- **Memory search**: Hybrid for user queries
- **Knowledge management**: Statements + knowledge entries

## Troubleshooting

### Model download fails
```bash
# Manual download
mkdir -p ~/.hypatia/models
# Download all-MiniLM-L6-v2 from HuggingFace
```

### No search results
1. Check shelf name matches project: `hypatia project show <name>`
2. Verify mining completed: check chunk count output
3. Use `hybrid` instead of `search` for better recall

### Project not found
```bash
# Check registry
hypatia project list

# Re-add if missing
hypatia project add <name> --root <path>
```
