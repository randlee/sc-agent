# Cross-Platform Guidelines

## Required Rules

1. Do not hardcode `/tmp` in production code or tests.
2. Use explicit paths, `std::env::temp_dir()`, or `tempfile::TempDir` for local
   file outputs.
3. Do not derive file paths from another product's home helpers or runtime
   roots; take them as explicit inputs.
4. Use `PathBuf` and `.join()` for path construction.
5. Any OS-specific transport or file behavior must be behind explicit cfg gates.
6. If stable Rust lacks the platform API needed to preserve parity, the shared
   docs must state the degraded guarantee explicitly instead of implying
   cross-platform equivalence.

## Current Platform Limitations

None recorded yet. Record each known platform gap here as it is found.
