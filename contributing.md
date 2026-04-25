# Contributing

Thanks for your interest in contributing!

## Getting Started

1. Fork the repository
2. Clone your fork
3. Create a feature branch: `git checkout -b feat/my-feature`
4. Make your changes

## Development

```bash
cargo build
cargo test
cargo clippy -- -D warnings
cargo fmt --check
```

All four must pass before submitting a PR.

Integration tests require Docker. They will be skipped automatically if the Docker socket is unavailable.

## Commit Messages

This project uses [Conventional Commits](https://www.conventionalcommits.org/):

- `feat:` — new feature
- `fix:` — bug fix
- `docs:` — documentation only
- `refactor:` — code change that neither fixes a bug nor adds a feature
- `test:` — adding or updating tests
- `chore:` — maintenance

## Pull Requests

- Keep PRs focused — one feature or fix per PR
- Include a clear description of what changed and why
- Ensure CI passes before requesting review

## Reporting Issues

Open a GitHub issue with:

- What you expected to happen
- What actually happened
- Steps to reproduce
- Environment details (OS, Docker version, cgroup version)

**Security issues are different** — do not file them as public issues. See [`SECURITY.md`](SECURITY.md) for the private disclosure path.

## License

By contributing, you agree that your contributions will be licensed under the [MIT License](license.md).
