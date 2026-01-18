# Contributing to hultra

First off, thank you for considering contributing to hultra! Your support and involvement are greatly appreciated. Below are some guidelines to help you get started.

---

## How Can I Contribute?

### 1. Reporting Bugs
If you encounter any bugs, please help us by reporting them. Here's how:
- Use the [Bug Report Template](https://github.com/pinpinroku/hultra/issues/new?template=bug_report.md) to submit a detailed bug report.
- Include steps to reproduce the bug, expected behavior, and screenshots if applicable.

### 2. Suggesting Features
Have an idea for a new feature? We'd love to hear it! Please:
- Use the [Feature Request Template](https://github.com/pinpinroku/hultra/issues/new?template=feature_request.md) to propose your idea.
- Provide as much detail as possible about the feature and how it will benefit the project.

### 3. Improving Documentation
If you find errors, typos, or areas in the documentation that can be improved:
- Fork the repository, make your changes, and submit a pull request.

### 4. Submitting Code Changes
If you want to contribute code, follow these steps:
1. **Fork** the repository.
2. **Clone** your fork to your local machine:
   ```bash
   git clone https://github.com/<your-username>/hultra.git
   cd hultra
   ```
3. **Create a new branch** for your changes:
   ```bash
   git checkout -b feature/your-feature-name
   ```
4. **Make your changes** and commit them with clear and concise commit messages:
   ```bash
   git add .
   git commit -m "Add feature: your-feature-name"
   ```
5. **Push your branch** to your fork:
   ```bash
   git push origin feature/your-feature-name
   ```
6. **Submit a pull request**:
   - Navigate to your fork on GitHub.
   - Click the "New Pull Request" button.
   - Provide a clear description of your changes.

---

### Commit Messages

This project uses [Gitmoji](https://gitmoji.dev/) for commit message formatting. Gitmoji provides a fun and visually clear way to describe the purpose of each commit.

While it's not mandatory for contributors to follow this format, we encourage you to try it out! Below are a few examples:

- 🐛 `:bug:` Fix a critical bug in feature X
- ✨ `:sparkles:` Add a new feature Y
- 📝 `:memo:` Update documentation for module Z

For more Gitmoji options, see the [Gitmoji Cheat Sheet](https://gitmoji.dev/).

If you're unfamiliar with Gitmoji or prefer using plain text commit messages, that's perfectly fine too. Clear and descriptive commit messages are always appreciated!

---

## Code Guidelines

- **Style**: `Cargo fmt`
- **Linting**: `Cargo clippy`
- **Testing**: Ensure your changes are accompanied by tests and that all tests pass:
  ```bash
  cargo test
  ```
- **Documentation**: Document your code where necessary (function-level comments).

---

## Pull Request Process

1. Make sure your PR is linked to an issue (if applicable).
2. Ensure your code passes all linting and CI checks.
3. Address any requested changes from maintainers promptly.
4. Be respectful and collaborative during the review process.

---

## Community Guidelines

- Be respectful to all contributors and maintainers.
- Ask questions and seek help in the [Discussions tab](https://github.com/pinpinroku/hultra/discussions).

---

Thank you for contributing to Everest Mod CLI! Together, we can make it even better.
