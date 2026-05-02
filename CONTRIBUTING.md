# Contributing to XFChess

Thank you for your interest in contributing to XFChess! We welcome contributions from everyone.

## I want to contribute code to XFChess

### Set up your development environment

1. Clone the repository
```bash
git clone https://github.com/trilltino/XFChess.git
cd XFChess
```

2. Install dependencies
- Rust toolchain (stable)
- Node.js 18+ (for web interface)
- Solana CLI (optional, for blockchain features)

3. Build the project
```bash
cargo build --release
```

4. Run the project
```bash
cargo run --release
```

For local development with monitoring:
```bash
scripts\run_offline.bat
```

### Pick a GitHub issue to work on

Look for issues tagged with:
- `good first issue` - suitable for newcomers
- `bug` - bug fixes
- `enhancement` - new features
- `documentation` - docs improvements

### Communicate with other devs on Discord

Join our [Discord](https://discord.gg/erZJCPCm) to discuss development and get help.

### Rules about AI-assisted code contributions

Generated AI code can be accepted, under some conditions:

- Carefully review and understand all the code you submit, and be able to explain it if asked
- Provide proof of manual testing of the changes, with screenshots or ideally a video
- Include in the pull-request message, or in commit messages, the prompts you used to generate the code, and the AI tool you used

### General guidelines for pull requests

- Explain why the change is needed, and what problem it solves
- Link to any relevant issues or discussions
- Prefer small, focused pull requests that only change one thing at a time
- Mark the pull request as a draft if you have not run the code
- Only mark the pull request as ready when you have confirmed that it works as intended - be on the lookout for edge cases
- Run tests: `cargo test`
- Ensure the code follows Rust best practices and idioms
- Add tests for new features
- Update documentation as needed

If you're unsure about something, or want to ask if the change is desired before doing the work, ask us in the programming channels on [Discord](https://discord.gg/erZJCPCm).

## I want to contribute artwork or documentation

Some issues that need artwork and documentation to be resolved are tagged `nontechnical`.

### Artwork
- 3D models and textures
- UI assets
- Icons and badges
- Sound effects

### Documentation
- Code documentation (doc comments)
- README improvements
- Tutorial guides
- API documentation

## I want to report a bug or a problem about XFChess

Make an issue. Before creating an issue, make sure that:

- You list the steps to reproduce the problem to show that other users may experience it as well, if the issue is not self-descriptive
- Search to make sure it isn't a duplicate. The advanced search syntax may come in handy
- It is not a trivial problem or demands unrealistic dev time to fix. Such issues may be closed
- Include system information (OS, Rust version, etc.)
- Include error messages and logs if applicable

## I want to suggest a feature for XFChess

Issue tickets on features that lack potential or effectiveness are not useful and may be closed. Discussions regarding whether a proposed new feature would be useful can be done on the [Discord](https://discord.gg/erZJCPCm) to gauge feedback.

When you're ready, make an issue ticket and link relevant, constructive comments regarding it in your issue ticket. Make sure the feature you propose:

- Is effective in delivering a goal. A feature that adds nothing new is purely fancy; please consider if it's truly necessary
- Doesn't rely on mundane assumptions. Non-technical people have the tendency to measure how difficult/easy a feature is to implement based on their unreliable instincts, and such assumptions waste everyone's time. Point out what needs to happen, not what you think will happen
- Is unique, if you're aiming to solve a problem. Features that can easily be replaced by easier ideas have little value and may not have to be brought up to begin with
- Is clear and concise. If ambiguities exist, define them or propose options

## I want to help translate XFChess

Check out our translation project on [Crowdin](https://crowdin.com/) (coming soon).

## Development Guidelines

### Code Style
- Follow Rust best practices and idioms
- Use `cargo fmt` to format code
- Use `cargo clippy` to catch common issues
- Keep functions focused and small
- Write descriptive variable and function names

### Testing
- Write unit tests for new functionality
- Write integration tests for complex features
- Test on multiple platforms if possible (Windows, macOS, Linux)
- Test Solana program changes on devnet before mainnet

### Commit Messages
- Use clear, descriptive commit messages
- Format: `type: description` (e.g., `feat: add tournament system`)
- Types: `feat`, `fix`, `docs`, `style`, `refactor`, `test`, `chore`

### Solana Development
- Test contract changes on devnet first
- Use `scripts/build_program.bat` to build the Solana program
- Verify contract logic with unit tests
- Check gas costs and rent requirements
- Document any breaking changes

## Other ways to contribute

- **Star the project** on GitHub
- **Share** XFChess with others
- **Report bugs** you encounter
- **Suggest improvements** on Discord
- **Help other users** with questions
- **Write blog posts** about XFChess
- **Create tutorials** and guides

See [https://xfchess.org/help/contribute](https://xfchess.org/help/contribute) for more ways to help.

Thank you for contributing to XFChess!
