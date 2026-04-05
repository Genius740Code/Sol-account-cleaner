# Contributing to Solana Recover

Thank you for your interest in contributing to Solana Recover! This guide will help you get started with contributing to the project, whether you want to fix bugs, add features, improve documentation, or help with testing.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Development Setup](#development-setup)
- [Contributing Guidelines](#contributing-guidelines)
- [Development Workflow](#development-workflow)
- [Testing](#testing)
- [Documentation](#documentation)
- [Pull Request Process](#pull-request-process)
- [Release Process](#release-process)

## Code of Conduct

This project follows the [Rust Code of Conduct](https://www.rust-lang.org/policies/code-of-conduct). Please be respectful and inclusive in all interactions.

## Development Setup

### Prerequisites

- **Rust**: 1.70.0 or newer
- **Git**: For version control
- **Docker**: Optional, for containerized development
- **Node.js**: 16+ (for frontend development, if applicable)

### Initial Setup

1. **Fork the Repository**
   ```bash
   # Fork the repository on GitHub, then clone your fork
   git clone https://github.com/YOUR_USERNAME/Sol-account-cleaner.git
   cd Sol-account-cleaner
   ```

2. **Add Upstream Remote**
   ```bash
   git remote add upstream https://github.com/Genius740Code/Sol-account-cleaner.git
   git fetch upstream
   ```

3. **Install Dependencies**
   ```bash
   # Install Rust dependencies
   cargo build
   
   # Install development tools
   cargo install cargo-watch cargo-audit cargo-deny
   ```

4. **Verify Setup**
   ```bash
   # Run tests to ensure everything works
   cargo test
   
   # Check code formatting
   cargo fmt --check
   
   # Run clippy for linting
   cargo clippy -- -D warnings
   ```

### Development Environment

#### IDE Setup

**VSCode** (Recommended):
- Install the [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=matklad.rust-analyzer) extension
- Install the [CodeLLDB](https://marketplace.visualstudio.com/items?itemName=vadimcn.vscode-lldb) extension for debugging
- Configure `.vscode/settings.json`:

```json
{
  "rust-analyzer.checkOnSave.command": "clippy",
  "rust-analyzer.cargo.loadOutDirsFromCheck": true,
  "rust-analyzer.procMacro.enable": true,
  "editor.formatOnSave": true,
  "files.watcherExclude": {
    "**/target/**": true
  }
}
```

**IntelliJ IDEA**:
- Install the [Rust plugin](https://plugins.jetbrains.com/plugin/8182-rust)
- Enable Rust toolchain in settings

#### Development Tools

```bash
# Watch for changes and recompile
cargo watch -x run

# Run with debug logging
RUST_LOG=debug cargo run

# Run specific example
cargo run --example basic_scan

# Run with features
cargo run --features "dev-tools"
```

## Contributing Guidelines

### Types of Contributions

We welcome various types of contributions:

1. **Bug Fixes**: Fix issues reported in the bug tracker
2. **New Features**: Add functionality that improves the project
3. **Documentation**: Improve README, API docs, and inline documentation
4. **Performance**: Optimize code for better performance
5. **Testing**: Add tests and improve test coverage
6. **Examples**: Create new examples and tutorials

### Before You Start

1. **Check Existing Issues**: Look for existing issues or pull requests
2. **Discuss Large Changes**: For major features, open an issue first
3. **Understand the Codebase**: Read the architecture documentation
4. **Follow Standards**: Adhere to existing code style and patterns

### Code Style

We use standard Rust formatting and linting tools:

```bash
# Format code
cargo fmt

# Run clippy
cargo clippy -- -D warnings

# Check for security vulnerabilities
cargo audit

# Check license compliance
cargo deny check
```

#### Code Organization

- Follow the existing module structure in `src/`
- Use meaningful variable and function names
- Add comments for complex logic
- Include examples in documentation

#### Documentation Standards

```rust
/// Scans a single wallet for recoverable SOL
/// 
/// # Arguments
/// 
/// * `wallet_address` - The Solana wallet address to scan
/// * `config` - Scanner configuration
/// 
/// # Returns
/// 
/// Returns a `WalletScanResult` containing:
/// - Total token accounts found
/// - Empty accounts eligible for recovery
/// - Estimated recoverable SOL
/// 
/// # Examples
/// 
/// ```rust
/// let result = scan_wallet("9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM", &config).await?;
/// println!("Recoverable SOL: {}", result.recoverable_sol);
/// ```
/// 
/// # Errors
/// 
/// This function will return an error if:
/// - The wallet address is invalid
/// - RPC connection fails
/// - Rate limits are exceeded
pub async fn scan_wallet(
    wallet_address: &str,
    config: &ScannerConfig,
) -> Result<WalletScanResult, ScanError> {
    // Implementation
}
```

## Development Workflow

### 1. Create a Branch

```bash
# Sync with upstream
git checkout main
git pull upstream main

# Create a feature branch
git checkout -b feature/your-feature-name

# Or for bug fixes
git checkout -b fix/issue-number-description
```

### 2. Make Changes

- Make small, focused commits
- Write clear commit messages
- Test your changes thoroughly

#### Commit Message Format

```
type(scope): description

[optional body]

[optional footer]
```

Types:
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation
- `style`: Code style (formatting, etc.)
- `refactor`: Code refactoring
- `test`: Adding or updating tests
- `chore`: Maintenance tasks

Examples:
```
feat(scanner): add batch processing for multiple wallets

Implement concurrent wallet scanning with configurable batch sizes
and rate limiting to improve performance for large-scale operations.

Closes #123
```

```
fix(rpc): handle connection timeout errors

Add proper error handling and retry logic for RPC timeouts
to improve reliability under high load.
```

### 3. Test Your Changes

```bash
# Run all tests
cargo test

# Run integration tests
cargo test --test integration

# Run with coverage (requires cargo-tarpaulin)
cargo tarpaulin --out Html

# Run benchmarks
cargo bench

# Check formatting and linting
cargo fmt --check
cargo clippy -- -D warnings
```

### 4. Update Documentation

- Update README.md if needed
- Add or update API documentation
- Update examples if you changed behavior
- Add changelog entry for significant changes

## Testing

### Test Structure

```
tests/
├── integration/          # Integration tests
├── common/              # Test utilities
└── fixtures/            # Test data

src/
├── lib.rs               # Library tests
└── **/mod.rs           # Module tests
```

### Writing Tests

#### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tokio_test;

    #[tokio::test]
    async fn test_wallet_scan_valid_address() {
        let config = ScannerConfig::default();
        let result = scan_wallet("9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM", &config).await;
        
        assert!(result.is_ok());
        let scan_result = result.unwrap();
        assert!(scan_result.total_accounts >= 0);
    }

    #[test]
    fn test_fee_calculation() {
        let wallet_info = WalletInfo {
            recoverable_lamports: 1_000_000_000,
            ..Default::default()
        };
        
        let fee_structure = FeeStructure {
            percentage: 0.15,
            minimum_lamports: 1_000_000,
            ..Default::default()
        };
        
        let fee = FeeCalculator::calculate_wallet_fee(&wallet_info, &fee_structure);
        assert_eq!(fee.fee_lamports, 150_000_000);
    }
}
```

#### Integration Tests

```rust
// tests/integration/api_tests.rs
use reqwest;
use serde_json::json;
use solana_recover::start_test_server;

#[tokio::test]
async fn test_api_health_check() {
    let server = start_test_server().await;
    let client = reqwest::Client::new();
    
    let response = client
        .get(&format!("{}/health", server.url()))
        .send()
        .await
        .unwrap();
    
    assert_eq!(response.status(), 200);
    
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["status"], "healthy");
}

#[tokio::test]
async fn test_api_single_scan() {
    let server = start_test_server().await;
    let client = reqwest::Client::new();
    
    let response = client
        .post(&format!("{}/api/v1/scan", server.url()))
        .json(&json!({
            "wallet_address": "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM"
        }))
        .send()
        .await
        .unwrap();
    
    assert_eq!(response.status(), 200);
    
    let result: WalletScanResult = response.json().await.unwrap();
    assert!(result.total_accounts >= 0);
}
```

### Test Coverage

We aim for high test coverage:

- **Unit Tests**: Test individual functions and methods
- **Integration Tests**: Test component interactions
- **API Tests**: Test HTTP endpoints
- **Performance Tests**: Benchmark critical paths

```bash
# Run tests with coverage
cargo tarpaulin --out Html --exclude-files "*/tests/*"

# View coverage report
open tarpaulin-report.html
```

## Documentation

### Types of Documentation

1. **API Documentation**: Inline docs for public APIs
2. **User Documentation**: Guides and tutorials
3. **Developer Documentation**: Architecture and internals
4. **Examples**: Working code examples

### Documentation Standards

- Use clear, concise language
- Include code examples
- Update documentation when code changes
- Use consistent formatting

### Running Documentation Checks

```bash
# Check documentation links
cargo doc --no-deps

# Test documentation examples
cargo test --doc

# Check for broken links in markdown
markdown-link-check *.md
```

## Pull Request Process

### Before Submitting

1. **Sync Your Branch**
   ```bash
   git fetch upstream
   git rebase upstream/main
   ```

2. **Run Full Test Suite**
   ```bash
   cargo test
   cargo fmt --check
   cargo clippy -- -D warnings
   ```

3. **Update Documentation**
   - Update relevant documentation
   - Add changelog entry if needed

### Creating the Pull Request

1. **Push to Your Fork**
   ```bash
   git push origin feature/your-feature-name
   ```

2. **Create PR on GitHub**
   - Use a clear title
   - Fill out the PR template
   - Link relevant issues
   - Add screenshots if applicable

### PR Template

```markdown
## Description
Brief description of changes and motivation.

## Type of Change
- [ ] Bug fix
- [ ] New feature
- [ ] Breaking change
- [ ] Documentation update

## Testing
- [ ] Unit tests pass
- [ ] Integration tests pass
- [ ] Manual testing completed
- [ ] Performance impact assessed

## Checklist
- [ ] Code follows project style guidelines
- [ ] Self-review completed
- [ ] Documentation updated
- [ ] Changelog updated
- [ ] No breaking changes (or documented)
```

### Review Process

1. **Automated Checks**: CI/CD pipeline runs tests
2. **Code Review**: Maintainers review the code
3. **Testing**: Reviewers may request additional tests
4. **Approval**: At least one maintainer approval required
5. **Merge**: Maintainers merge the PR

## Release Process

### Versioning

We use [Semantic Versioning](https://semver.org/):
- **MAJOR**: Breaking changes
- **MINOR**: New features (backwards compatible)
- **PATCH**: Bug fixes (backwards compatible)

### Release Checklist

1. **Update Version**
   ```bash
   # Update Cargo.toml
   cargo bump patch  # or minor/major
   ```

2. **Update Changelog**
   - Add release notes
   - Document breaking changes
   - Credit contributors

3. **Tag Release**
   ```bash
   git tag -a v1.0.0 -m "Release v1.0.0"
   git push upstream v1.0.0
   ```

4. **Publish**
   ```bash
   # Publish to crates.io (if applicable)
   cargo publish
   
   # Create GitHub release
   # Upload binaries and documentation
   ```

## Getting Help

If you need help with contributing:

- 📧 **Email**: dev@solana-recover.com
- 💬 **Discord**: [Developer Channel](https://discord.gg/solana-recover-dev)
- 🐛 **Issues**: [GitHub Issues](https://github.com/Genius740Code/Sol-account-cleaner/issues)
- 📖 **Documentation**: [Developer Guide](docs/developer-guide.md)

## Recognition

Contributors are recognized in:
- README.md contributors section
- Release changelogs
- Annual contributor highlights

Thank you for contributing to Solana Recover! Your contributions help make the project better for everyone.
