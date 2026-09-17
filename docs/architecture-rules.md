English | [Português (Brasil)](pt-BR/architecture-rules.md) | [Español](es/architecture-rules.md)

# Architecture rules

Read this before writing or moving code, and check a change against it before committing. When a rule and
the code disagree, the rule wins for new code. Existing code is reorganized when it is next touched, not in
one big rewrite. Change a rule only with a dated note, as `decisions.md` does.

## 1. The simplest thing that works (ponytail)

Stop at the first rung that holds:

1. Does it need to exist? Skip speculative needs.
2. Is it already in this codebase? Reuse it.
3. Does the standard library do it?
4. Does an installed dependency do it? Add a new one only when a few lines cannot.
5. Only then write the minimum code that works.

- No abstraction with one implementation, no factory for one product, no configuration nobody sets, and no
  scaffolding "for later".
- A deliberate shortcut with a known ceiling gets a `// ponytail:` comment naming the ceiling and the
  upgrade path.
- Understand the problem first: read the code a change touches and trace the flow. A bug fix goes where
  every caller routes through, not only where the symptom showed.
- Non-trivial logic (a parser, a branch-heavy rule, security or money) leaves one small runnable test.
- Before every commit, review the diff for over-engineering (ponytail review) and cut what it finds.

## 2. Workspace layers

- `crates/` holds libraries and `apps/` holds binaries. Dependencies point downward only.
- **Format crates are pure** (`ue2-*`, `l2-dat`, `canastra-data`, `canastra-protocol`, `canastra-ui`):
  bytes or text in, types out. No file system, network, clock or threads, so they test against synthetic
  input.
- **IO lives at the edges**: `canastra-net` (connections), `canastra-db` (PostgreSQL), and the apps.
- **Apps are thin**: `main.rs` parses arguments and starts things. Each responsibility of the app lives in
  its own module.
- Client, server and Studio share the same domain crates. A type is defined once; nothing is copied
  between sides.

## 3. Folders by context inside a crate

- A crate's `src/` is organized by **context** (what the code is about), not dumped flat:
  - `src/scene/` for the 3D scene, `src/network/` for connections, `src/screens/` for UI screens;
  - in a server, `src/players/`, `src/registry/`, `src/world/`, `src/characters/`.
- A context folder has a `mod.rs` that states in its doc comment what the context owns and exposes a
  small API. Its files hold one focused responsibility each.
- A context that fits in one file is that file (`class.rs`). It becomes a folder as soon as it needs a
  **second file**; never spread one context over several flat files in `src/`.
- Shared helpers go in the context that owns them. A `utils` or `common` grab bag is not a context.
- Tests sit next to the code: `#[cfg(test)] mod tests` in the file, or `tests.rs` inside the context
  folder for end-to-end tests of that context.

## 4. Files and functions

- A file holds one responsibility. Split it when it passes about 200 to 250 lines or starts doing two
  things.
- Clippy runs pedantic with `-D warnings`. Untrusted input never panics: no `unwrap`, `expect`, indexing
  or `panic` outside tests.
- Doc comments say what a thing is for and any non-obvious rule; the code says how. Match the comment
  density of the surrounding code.
- Names are full words in English. Code, comments and docs are in English; conversation with the user is
  in Portuguese.

## 5. Data and formats

- **Domain model, not a legacy mirror.** Merge and restructure legacy data into what the game means.
  Store a value once: when legacy files repeat it (a child class copying its starting class), the model
  keeps it in one place.
- Migrations account for every legacy field: mapped, or ignored with a stated reason. Unmodeled server
  data is counted in the report.
- Serialized layouts are frozen by tests. Changing a type means bumping its `VERSION`
  (`canastra_data::format`, `canastra_protocol`) and re-blessing the fixture.
- Configuration is TOML per binary, with secrets overridable by environment variables. Every app ships an
  `*.example.toml` documenting each setting; real configs are git-ignored.

## 6. Security and networking

- Players reach servers only through `canastra-net` (Noise, pinned server key). Peer servers use
  authorized keys.
- The server validates every action. Clients are untrusted input.
- Passwords use Argon2id. Account existence never leaks through messages or timing.
- Limits live at every edge: frame size, handshake timeout, attempt rate limits, capacity.

## 7. Verification and commits

- Check against the real thing: the H5 client files, a running server, a screenshot or a measurement.
  Unit tests alone do not prove a feature works.
- Database tests are `#[ignore]` and run with `CANASTRA_TEST_DATABASE_URL`.
- Commit each finished step with Conventional Commits (`type(scope): summary`, as in `git log`), a prose
  body saying what changed and why, and no trailers.
- Push after each commit.
