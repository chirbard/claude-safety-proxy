# claude-safety-proxy

A local proxy that sits between Claude Code and Anthropic's API. Every outgoing request is scanned by a local Ollama model before it leaves your machine. If secrets or PII are detected, the request is blocked and Claude Code is told what was found.

## How it works

```
Claude Code → localhost:49733 → (scan) → api.anthropic.com
```

Scans: API keys, passwords, private keys, connection strings, PII. Uses `qwen3.5:2b` via Ollama — nothing leaves your machine for scanning.

## Requirements

- [Ollama](https://ollama.com) running locally
- The configured model pulled: `ollama pull qwen3.5:2b`

## Setup

```bash
cargo build --release
```

Point Claude Code at the proxy:

```bash
export ANTHROPIC_BASE_URL=http://localhost:49733
```

Run the proxy:

```bash
cargo run --release
```

## Configuration

Edit `config.toml`:

```toml
port = 49733
ollama_url = "http://localhost:11434"
model = "qwen3.5:2b"
max_chunk_chars = 4000
```

Or set `CONFIG=/path/to/config.toml` to use a different file.
