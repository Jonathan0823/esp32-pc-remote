# Contributing

PRs and issues welcome.

## Guidelines

- Keep `config.h` out of commits (it's gitignored)
- Copy `config.example.h` to `config.h` if it does not exist, then run `pio run` and `./scripts/test_host.sh` before submitting
- Use conventional commits: `feat:`, `fix:`, `refactor:`, `docs:`, etc.
- One logical change per commit
