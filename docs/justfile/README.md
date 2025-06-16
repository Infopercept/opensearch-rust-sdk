This `justfile` provides convenient shortcuts for development workflows. Here's how to use it:

## Installation & Setup

### 1. Install `just` Command Runner

```bash
# Using cargo (Rust package manager)
cargo install just

# Or using Homebrew (macOS/Linux)
brew install just

# Or using package managers
# Ubuntu/Debian: sudo apt install just
# Arch: sudo pacman -S just
```

### 2. Set Environment Variables

Create a `.env` file in the project root:

```bash
# .env file
PASS=YourSecurePassword123!
```

## Available Commands

### Core Development Workflow

```bash
# Format code and build
just build

# Run tests (alias: just t)
just test

# Start the extension server
just run

# Format code only
just fmt

# Run linting
just clippy
```

### OpenSearch Container Management

```bash
# Build custom OpenSearch image with extensions enabled
just buildos          # x64 architecture
just buildosarm        # ARM64 architecture

# Run OpenSearch container (finch/Docker)
just runos             # Direct run (currently has version issues)
just runosdocker       # Interactive shell in container
just runosdockerarm    # ARM64 version
```

### Extension Management

```bash
# Register extension with OpenSearch (insecure)
just loadext

# Register extension with authentication
just loadext_secure

# Test extension endpoint
just getext
```

## Complete Development Workflow## Command Reference

### Development Commands

| Command | Purpose | Equivalent |
|---------|---------|------------|
| `just build` | Format code then build | `cargo fmt --all && cargo build` |
| `just test` (or `just t`) | Run test suite | `cargo test` |
| `just run` | Start extension server | `cargo run` |
| `just fmt` | Format code only | `cargo fmt --all` |
| `just clippy` | Run linter | `cargo clippy --all --all-targets --all-features` |

### Container Commands

| Command | Purpose | Notes |
|---------|---------|-------|
| `just buildos` | Build x64 OpenSearch image | Uses custom Dockerfile with extensions enabled |
| `just buildosarm` | Build ARM64 OpenSearch image | For Apple Silicon/ARM processors |
| `just runos` | Run OpenSearch container | **Currently broken** - version mismatch |
| `just runosdocker` | Interactive OpenSearch shell | For debugging/development |

### Extension Commands

| Command | Purpose | When to Use |
|---------|---------|-------------|
| `just loadext` | Register extension (insecure) | Development with docker-compose-dev.yml |
| `just loadext_secure` | Register extension (secure) | Production with authentication |
| `just getext` | Test extension endpoint | Verify extension is working |

## Typical Development Session

```bash
# 1. Start fresh development session
just build                    # Format and build
just test                     # Ensure tests pass

# 2. Start services
cd resources/compose/
docker-compose -f docker-compose-dev.yml up -d
cd ../../

# 3. Start extension
just run &                    # Run in background

# 4. Register extension
sleep 10                      # Wait for startup
just loadext                  # Register with OpenSearch

# 5. Test
just getext                   # Test extension endpoint

# 6. Development iterations
# Edit code...
just build                    # Rebuild
# Kill and restart extension server as needed
```

## Environment Configuration

The justfile uses `dotenv-load := true`, so create a `.env` file:

```bash
# .env
PASS=YourSecurePassword123!
```

This password is used for:
- `loadext_secure` - Authenticated extension registration
- `getext` - Testing secure endpoints
- Container authentication

## Troubleshooting

**If `just` commands fail:**

```bash
# Check just installation
just --version

# List available commands
just --list

# Run with verbose output
just --verbose build
```

**For container issues:**

The `runos` command is currently broken due to version mismatches (needs OpenSearch 3.x but only 2.x images available). Use docker-compose instead:

```bash
# Instead of: just runos
cd resources/compose/
docker-compose -f docker-compose-dev.yml up -d
```

**For extension registration issues:**

```bash
# Check if OpenSearch is running
curl http://localhost:9200/_cluster/health

# Check if extension server is running
netstat -an | grep 1234

# Use verbose curl for debugging
curl -v -XPOST "http://localhost:9200/_extensions/initialize" \
  -H "Content-Type:application/json" \
  --data @examples/hello/hello.json
```

The justfile significantly streamlines the development workflow by reducing complex multi-command operations to simple, memorable shortcuts.
