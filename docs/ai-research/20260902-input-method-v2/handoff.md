# Research Handoff: Optional Wayland Input Method

## Goal

Implement optional direct UTF-8 insertion through Wayland input-method-v2 with
safe capability negotiation and existing backend fallbacks.

## Current conclusion

The optional backend is implemented in the worktree as explicit
`non_bmp_input: input_method` behavior. It is deliberately excluded from
`auto` because input-method-v2 is exclusive per seat and Omarchy's Fcitx5 owns
that role. Direct commit uses active state only, atomically deletes the trigger
and inserts UTF-8, falls back only before commit, and refuses unsafe replay
after an indeterminate dispatch failure. Deterministic Rust tests cover config
opt-in, application fallback choice, and activation/deactivation lifecycle.
The input-method client is independent of the selected keyboard transport, so
direct commit can coexist with either virtual-keyboard-v1 or uinput fallback.

## Validation

- `cargo fmt --check`
- `cargo test --frozen`: 80 unit tests and 5 pack CLI tests passed
- `cargo clippy --frozen --all-targets --all-features -- -D warnings`
- `cargo audit --no-fetch`: no vulnerabilities reported
- `cargo run --frozen -- --help`
- `git diff --check`

No live desktop test, service restart, push, or release has been performed.

## Important files

- `source-ledger.md`
- `findings.md`
- `gaps.md`
- `src/injector.rs`
- `src/daemon.rs`
- `src/config.rs`

## Resume prompt

Read this handoff and source ledger first. Review the worktree diff and run the
non-interactive validation workflow. Do not inject input, mutate the clipboard,
change focus, stop Fcitx5, restart the service, or launch applications without
explicit user approval.
