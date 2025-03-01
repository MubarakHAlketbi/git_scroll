# Git Scroll

A visualization tool for Git repositories, optimized for LLM context.

## Overview

Git Scroll is a desktop application that helps you visualize Git repository structures and export them in formats optimized for Large Language Models (LLMs). It provides an interactive, zoomable interface to explore repositories and understand their structure.

![Git Scroll Screenshot](docs/screenshot.png) *(Screenshot will be added in the future)*

## Features

- **Repository Visualization**: Interactive squares representing directories and files
- **Zoom Levels**: Multiple levels of detail, from high-level overview to file content
- **Filtering**: Built-in filters for common directories to ignore (node_modules, .git, etc.)
- **Export Options**: Export repository structure in formats optimized for LLMs
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
2. Click "Clone" to clone and analyze the repository
3. Use the zoom controls to adjust the level of detail
4. Click on directories to explore their contents
5. Export the repository structure for use with LLMs

## Project Structure

```
git_scroll/
├── src/
│   ├── main.rs           # Application entry point
│   ├── app.rs            # Main application state
│   ├── git/              # Git operations
│   │   └── mod.rs        # Git module implementation
│   ├── directory/        # Directory parsing
│   │   └── mod.rs        # Directory module implementation
│   ├── visualization/    # Rendering and visualization
│   │   └── mod.rs        # Visualization module implementation
│   └── ui/               # User interface components
│       └── mod.rs        # UI module implementation
└── Cargo.toml            # Dependencies and build configuration
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