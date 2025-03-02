# Git Scroll

A file listing and token counting tool for Git repositories, optimized for LLM context.

## Overview

Git Scroll is a desktop application that analyzes Git repositories by providing a sortable table of files with their token counts. This tool helps developers understand token usage across a codebase, making it ideal for preparing data for Large Language Models (LLMs).

![Git Scroll Screenshot](docs/screenshot.png) *(Screenshot will be added in the future)*

## Features

- **Sortable File List**: Table with columns for index, file name, and token count, sortable by each column
- **Token Counting**: Counts tokens (words) in text files to assess LLM context size
- **Total Row**: Displays total files and tokens at the bottom of the list
- **Filtering**: Ignores common directories (e.g., `.git`, `node_modules`)
- **Statistics Panel**: Shows total files, total tokens, and top files by token count
- **Cross-Platform**: Works on Windows, macOS, and Linux

## Installation

### Prerequisites

- Rust toolchain (rustc, cargo) - install via [rustup](https://rustup.rs/)
- Git - for version control and repository operations

### Building from Source

1. Clone the repository:
   ```
   git clone https://github.com/yourusername/git_scroll.git
   cd git_scroll
   ```

2. Build the project:
   ```
   cargo build --release
   ```

3. Run the application:
   ```
   cargo run --release
   ```
## Usage

1. Enter a Git repository URL in the input field
2. Click "Clone" to fetch and analyze the repository
3. View the sortable file list with token counts
4. Click column headers to sort by index, name, or tokens
5. Use the settings panel to change sort options or apply filters
6. Check the statistics panel for token usage insights

## Project Structure

```
git_scroll/
├── src/
│   ├── main.rs      # Application entry point
│   ├── app.rs       # Main application state and table view
│   ├── git/         # Git operations
│   │   └── mod.rs   # Git module implementation
│   ├── directory/   # Directory parsing
│   │   └── mod.rs   # Directory module implementation
│   └── ui/          # User interface components
│       └── mod.rs   # UI module implementation
└── Cargo.toml       # Dependencies and build configuration
```

## Development

### Running Tests

```
cargo test
```

### Running with Debug Information

```
cargo run --features debug
```

### Building for Release

```
cargo build --release
```

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Acknowledgments

- [eframe](https://github.com/emilk/egui/tree/master/eframe) - Cross-platform GUI framework
- [git2-rs](https://github.com/rust-lang/git2-rs) - Rust bindings to libgit2