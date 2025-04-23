# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build/Test Commands
- Build workspace: `cargo build --workspace`
- Run all tests: `cargo test --workspace`
- Run single test: `cargo test test_name`
- Run specific module tests: `cargo test --package mdbook-i18n-helpers --lib -- module_name::tests`
- Lint: `cargo clippy --all-targets`
- Format: `dprint fmt`

## Code Style Guidelines
- Formatting: Use dprint (configured in dprint.json)
- Imports: Group by namespace (std, external, internal), alphabetical within groups
- Naming: snake_case for functions, PascalCase for types, SCREAMING_SNAKE_CASE for constants
- Error handling: Use anyhow with .context() for error propagation
- Documentation: Use doc comments (///) for public APIs with examples
- Pattern matching: Prefer modern let-else patterns over matches! macro
- Testing: Include tests in same file as the code they test with #[cfg(test)]

## Project Structure
- Workspace with multiple crates: i18n-helpers (core), i18n-report, mdbook-tera-backend
- Fuzzing in separate fuzz directory
- Internationalization helpers for mdbook using Gettext for translation