# CmdSage

Rust CLI that maps natural-language requests to executable shell command templates using keyword and semantic matching.

## Highlights

- Interactive and one-shot CLI modes for command discovery and execution
- Keyword and optional semantic matching over a local command knowledge base
- Local-first developer tool with configuration and history support

## Tech Stack

- Rust, clap, jieba-rs, ONNX Runtime, tokenizers, crossterm, dialoguer

## Repository Layout

- `src/main.rs`
- `commands`
- `Cargo.toml`
- `install.sh`

## Getting Started

- Build and install with `./install.sh`, or run `cargo build --release` manually.
- Place command templates in the `commands/` directory or the installed `~/.cmdsage/commands` path.
- Run `cmdsage "your natural language request"` after installation.

## Current Status

- This README was refreshed from a code audit and is intentionally scoped to what is directly visible in the repository.
