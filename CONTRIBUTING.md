English | [Português (Brasil)](CONTRIBUTING.pt-BR.md) | [Español](CONTRIBUTING.es.md)

# Contributing to CanastraEngine

Thanks for helping build CanastraEngine. This guide explains how the project works so your time turns into
changes that get merged. Questions are welcome in the issues, in English or Portuguese.

## Before you start

1. Set up your machine with [docs/getting-started.md](docs/getting-started.md).
2. Read the [architecture rules](docs/architecture-rules.md). Every change is checked against them.
3. Skim [docs/decisions.md](docs/decisions.md) for what has been agreed and the build order.
4. Look for an open issue, or open one describing what you want to do before starting larger work, so it
   does not clash with something in progress.

Read the [code of conduct](CODE_OF_CONDUCT.md) too; it applies everywhere the project lives.

## Hard rules

- **Never commit Lineage II client files**, extracted assets, or anything taken from leaked or proprietary
  sources. The engine reads an installed client in place; the repository holds only our own work.
- **Keep the licensing boundary.** Only migration tooling may depend on `crates/l2-dat-h5`. The client,
  servers and Studio must not.
- **No secrets.** Real `canastra-login.toml` and `canastra-game.toml` files, keys and passwords stay out
  of the repository; only the `*.example.toml` files are committed.
- **Clients are untrusted.** Server code validates everything a player sends.

## How work flows

1. Fork the repository and create a branch from `master`, named after the change:
   `feat/character-select-camera`, `fix/bsp-zero-length-surfaces`.
2. Make one focused change per pull request. Several small pull requests are reviewed faster than one
   large one.
3. Enable the guards once per clone, so every commit is checked:

   ```
   git config core.hooksPath .githooks
   ```

   The hook runs `cargo fmt --check`, `cargo clippy -D warnings` and `cargo test`. CI runs the same checks
   and a pull request cannot be merged while they fail.
4. Open a pull request against `master` and fill in its template. At least one maintainer review is
   required, and conversations must be resolved before merging.

## Writing code

- **The simplest thing that works.** No abstraction with one implementation, no configuration nobody sets,
  no code "for later". The [architecture rules](docs/architecture-rules.md) spell this out.
- **Organize by context.** A crate's `src/` is split into folders by what the code is about, not dumped
  flat. Files stay around 200 to 250 lines at most.
- **Pure format crates.** `ue2-*`, `l2-dat`, `canastra-data`, `canastra-protocol` and `canastra-ui` take
  bytes or text and return types, with no file system, network or clock.
- **No panics on untrusted input.** `unwrap`, `expect`, indexing and `panic!` are denied outside tests,
  because client files and network messages can be anything.
- **Leave one test behind** for non-trivial logic: a parser, a rule with branches, anything touching
  security.
- **English** for code, comments and documentation.

## Proving it works

Unit tests alone do not prove a feature works. Check against the real thing:

- For file formats, run the `canastra` tools (`scan`, `level`, `mesh`, `migrate`) on a real High Five
  client and include the numbers in the pull request.
- For anything visible, attach before and after screenshots, and an H5 screenshot when the goal is to match
  the original client.
- For servers, describe the end-to-end run: what you started, what you did in the client, and what the logs
  showed.

## Commit messages

Use [Conventional Commits](https://www.conventionalcommits.org): `type(scope): summary`, where the scope is
the crate or area, as in `git log`. Follow the summary with a body in prose that explains what changed and
why, and how it was verified.

```
fix(ue2-level): skip BSP nodes whose references lead nowhere

One map of the client stores nodes whose surface index is -1, which failed the
whole level. Such nodes are now left out. All 208 maps read.
```

Common types: `feat`, `fix`, `refactor`, `docs`, `test`, `chore`.

## Reporting bugs and ideas

Open an issue with the matching template. For bugs, include the steps, what you expected, what happened,
your operating system and GPU, and logs or screenshots. Security problems do not go in issues; see
[SECURITY.md](SECURITY.md).

## License of contributions

By contributing, you agree that your work is licensed under the [Canastra Source License](LICENSE).
In short: CanastraEngine may be used, modified and shared for free, and servers built with it may earn
money, but the engine itself may not be sold. Extensions you write are yours and may be sold.
