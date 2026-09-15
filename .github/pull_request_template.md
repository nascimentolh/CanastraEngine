## What and why

<!-- What does this change, and why is it needed? Link the issue it closes, if any. -->

## How it was verified

<!-- Unit tests alone are not enough. Say what you ran against the real client or servers and what you
saw: tool output, before and after screenshots, an H5 screenshot to compare, server logs. -->

## Checklist

- [ ] One focused change, following [the architecture rules](../docs/architecture-rules.md)
- [ ] `cargo fmt`, `cargo clippy -D warnings` and `cargo test` pass (the pre-commit hook runs them)
- [ ] Non-trivial logic has a test
- [ ] Commits follow Conventional Commits with a prose body
- [ ] Documentation updated where behavior changed
- [ ] No Lineage II client files, leaked material, keys or real configuration files
