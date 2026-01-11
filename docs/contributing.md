# Contributing to BlazeBee

We welcome contributions from the community! This guide outlines how to contribute to BlazeBee development, report issues, and submit improvements.

## Getting Started

### Prerequisites

To contribute to BlazeBee, you'll need:

- Rust 1.86+ installed
- Git for version control
- Docker (for testing container builds)
- A Linux system for development and testing

### Setting Up Your Development Environment

1. Fork the BlazeBee repository on GitHub
2. Clone your fork locally:
   ```bash
   git clone https://github.com/rubtsov-stan/blazebee.git
   cd blazebee
   ```
3. Install dependencies:
   ```bash
   # Install Rust if not already installed
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   
   # Install additional tools
   rustup component add clippy rustfmt
   ```
4. Build the project:
   ```bash
   cargo build
   ```

## Understanding the Codebase

### Project Structure

```
blazebee/
├── Cargo.toml           # Main project manifest
├── Makefile             # Build automation
├── Dockerfile           # Container build definition
├── config.example.toml  # Example configuration
├── src/                 # Main source code
│   ├── main.rs          # Entry point
│   ├── lib.rs           # Library exports
│   ├── config/          # Configuration handling
│   └── core/            # Core logic
│       ├── collectors/  # Metric collectors
│       ├── executor.rs  # Collection executor
│       └── readiness.rs # Health checking
├── crates/              # Workspace members
│   └── mqtt/v4/         # MQTT transport implementation
└── docs/                # Documentation
```

### Key Concepts

- **Collectors**: Individual metric gathering units that implement the `Collector` trait
- **Registry**: Central registry for available collectors
- **Executor**: Orchestrates the collection process
- **Publisher**: Abstracts transport mechanisms (currently MQTT)
- **Features**: Compile-time flags that enable/disable functionality

## Ways to Contribute

### Bug Reports

When reporting bugs, please include:

- Detailed description of the issue
- Steps to reproduce
- Expected vs actual behavior
- System information (OS, architecture, Rust version)
- Relevant logs or error messages

### Feature Requests

Feature requests should include:

- Clear problem statement
- Proposed solution
- Use cases and benefits
- Potential implementation approaches

### Code Contributions

#### Adding New Collectors

New collectors should:

1. Implement the `Collector` trait
2. Register themselves in the collector registry
3. Include proper error handling
4. Follow existing code style and patterns
5. Include unit tests where appropriate

Example collector structure:
```rust
use async_trait::async_trait;
use serde::Serialize;

use crate::core::collectors::{Collector, CollectorResult};

pub struct MyCollector;

#[async_trait]
impl Collector for MyCollector {
    type Output = MyMetrics;
    
    async fn collect(&self) -> CollectorResult<Self::Output> {
        // Implementation here
    }
}

#[derive(Serialize)]
pub struct MyMetrics {
    // Metric fields
}
```

#### Improving Existing Collectors

Improvements to existing collectors might include:

- Performance optimizations
- Better error handling
- Additional metrics
- Improved parsing of system files
- Cross-platform compatibility

#### Documentation

Documentation contributions are always welcome:

- Improving existing documentation
- Adding examples
- Clarifying complex concepts
- Correcting typos or outdated information

## Development Workflow

### Making Changes

1. Create a new branch for your work:
   ```bash
   git checkout -b feature/my-feature
   ```
2. Make your changes following the code style
3. Test your changes thoroughly
4. Update documentation if needed
5. Run linters and tests:
   ```bash
   cargo fmt --all
   cargo clippy --all-targets --all-features
   cargo test
   ```
6. Commit your changes with a clear message:
   ```bash
   git add .
   git commit -m "Add feature: description of what was added"
   ```

### Testing

#### Unit Tests

Run unit tests to ensure your changes don't break existing functionality:
```bash
cargo test
```

#### Integration Tests

If you've made significant changes, test with:
```bash
# Build and run manually
cargo run -- -c config.example.toml

# Or use Docker to test container builds
make docker
make docker-test
```

#### Cross-Platform Testing

Test on different platforms if your changes affect platform-specific functionality:
```bash
# Build for different architectures
make docker ARCH=arm64 PLATFORM=linux/arm64
```

### Submitting Changes

1. Push your branch to your fork:
   ```bash
   git push origin feature/my-feature
   ```
2. Open a pull request against the main repository
3. Fill out the pull request template
4. Address review feedback
5. Wait for approval and merge

## Code Style Guidelines

### Rust Style

- Follow the Rust API Guidelines
- Use `cargo fmt` for consistent formatting
- Use `cargo clippy` for linting
- Write meaningful variable and function names
- Document public APIs with doc comments

### Naming Conventions

- Use snake_case for functions and variables
- Use PascalCase for types and traits
- Use SCREAMING_SNAKE_CASE for constants
- Prefix private helper functions with `_` if they're unused in the current scope

### Error Handling

- Use proper error types with `thiserror`
- Provide meaningful error messages
- Handle errors gracefully without crashing
- Log errors appropriately

## Community Guidelines

### Communication

- Be respectful and constructive
- Ask questions when unsure
- Provide helpful feedback during reviews
- Be patient with newcomers

### Review Process

Pull requests will be reviewed for:

- Code quality and style
- Functionality and correctness
- Performance implications
- Security considerations
- Documentation completeness

## Getting Help

If you need help:

- Open an issue for technical questions
- Check the documentation
- Look at existing implementations for reference
- Reach out to maintainers via GitHub

Thank you for considering contributing to BlazeBee! Your efforts help make the project better for everyone.