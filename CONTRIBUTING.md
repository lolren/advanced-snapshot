# Contributing

Contributions are welcome when they preserve truthful capability reporting,
mobile usability and upstream maintainability.

## Development rules

1. Base a change on the documented known-good upstream tag.
2. Keep lower-stack, transport and UI changes in separate commits.
3. Do not expose a switch or slider until the active camera advertises the
   corresponding control and the result can be validated.
4. Include a fallback path for fixed-focus cameras and ordinary webcams.
5. Run formatting, Rust tests, Meson validation and a staged install.
6. For phone-camera changes, record bounded tests for every affected sensor and
   preserve an exact rollback package before installation.

Do not submit proprietary camera libraries, decoded vendor tuning, photographs,
raw captures, device identifiers, credentials or unsanitized logs.

The original Snapshot project remains the right destination for generic fixes
that do not depend on this fork's downstream camera stack. Attribution must be
preserved when moving a change in either direction.
