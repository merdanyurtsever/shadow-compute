# shadow-compute

Basic GPU-accelerated image processing over a local Unix socket.

This project runs a background daemon that receives tasks and executes image processing on an integrated GPU using `wgpu` (Vulkan backend). The current compute shader converts image pixels to grayscale.

## What It Does

- Starts a daemon on `/tmp/shadow_compute.sock`
- Accepts serialized tasks via Unix socket (`bincode` + `serde`)
- Decodes images on CPU (Rayon + TurboJPEG + memory mapping)
- Runs a WGSL compute shader on GPU
- Prints throughput telemetry for dataset processing

## Project Layout

- `src/bin/daemon.rs`: socket server and task dispatcher
- `src/bin/cli.rs`: sends one task to daemon
- `src/gpu.rs`: GPU setup and dataset processing pipeline
- `src/shader.wgsl`: grayscale compute shader
- `src/lib.rs`: shared `Task` enum

## Requirements

- Linux (uses Unix domain socket)
- Rust toolchain (edition 2024)
- Vulkan-capable GPU/driver available to `wgpu`

## Build

```bash
cargo build --release
```

## Run

Start daemon in one terminal:

```bash
cargo run --release --bin daemon
```

Send a task from another terminal:

```bash
cargo run --release --bin cli
```

## Current Task Types

Defined in `src/lib.rs`:

- `Ping { message: String }`
- `ProcessImage { path: String }` (placeholder in daemon)
- `ProcessDataset { dir_path: String }`

## Important Notes

- The CLI currently sends a hardcoded dataset path from `src/bin/cli.rs`.
- Daemon reads up to 1024 bytes per socket read; very large payloads are not handled yet.
- Processing is benchmark-focused and currently reports telemetry rather than writing output images.

## Troubleshooting

- If CLI cannot connect, ensure daemon is running and `/tmp/shadow_compute.sock` exists.
- If GPU init fails, verify Vulkan drivers and permissions.
- If no images are found, check dataset path and file extensions (`jpg`, `jpeg`, `png`).

## Development

Run quick checks:

```bash
cargo check
```

Format and lint (optional but recommended):

```bash
cargo fmt
cargo clippy --all-targets -- -D warnings
```
