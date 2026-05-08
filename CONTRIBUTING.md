# Contributing to Treadlink

Thanks for your interest in contributing! This guide covers how to set up the project, coding conventions, and the PR workflow.

## Development Setup

1. Install Rust via [rustup](https://rustup.rs/).
2. Add the appropriate target:
   ```sh
   rustup target add thumbv7em-none-eabihf        # nRF52840
   ```
3. Install [probe-rs](https://probe.rs/) for flashing/debugging nRF boards.
4. Clone the repo and build:
   ```sh
   git clone <repo-url>
   cd treadlink
   cargo build --release
   ```

## Code Style

- Rust, `no_std`, embedded — keep allocations to zero.
- Use `embassy` async tasks for concurrency; avoid busy-loops.
- Run `cargo clippy` and `cargo fmt` before committing.
- Keep unsafe blocks minimal and well-commented with `// SAFETY:` explanations.

## Project Conventions

- Board-specific code lives in `src/board/`. Shared logic should be hardware-agnostic.
- BLE protocol handling goes in `src/ble/`.
- Data conversion (FTMS → RSC) lives in `src/conversion/`.

## Branching & PRs

- Work on feature branches off `main`.
- Keep PRs focused — one feature or fix per PR.
- Write a clear description of what changed and why.
- Ensure `cargo build --release` succeeds before opening a PR.

## Testing

Embedded testing is limited, but:
- Unit-test conversion logic and protocol parsing with `#[cfg(test)]` modules where possible.
- Integration testing happens on hardware — document what you tested and on which board.

## AI-Assisted Development

This project includes Rust-focused AI skills (in `.kiro/` and `.agents/skills/`) to assist with coding. If you're using Kiro or a similar AI tool, these skills provide context about embedded Rust patterns, Embassy, and BLE conventions used in this project.

## Questions?

Open an issue or start a discussion. We're a small project — informal communication is fine.
