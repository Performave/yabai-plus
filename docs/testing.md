# Testing

This fork has two automated test layers.

## Unit tests

```bash
make test
```

The unit harness lives under `tests/` and builds a standalone binary that includes
the source tree with `TESTS` defined. These tests should stay deterministic and
avoid depending on a running window manager, Accessibility permission, a GUI
session, the scripting addition, SIP state, or Mission Control.

CI runs `make` and `make test` on a GitHub-hosted macOS runner.

## Local e2e smoke

```bash
make e2e
```

The e2e smoke test builds `bin/yabai`, starts it in the foreground with an empty
temporary config, sends real `yabai -m` messages, and verifies basic query,
config, rule, and error-path behavior.

Stop the normal service first if you want the smoke test to run instead of skip:

```bash
yabai --stop-service
make e2e
yabai --start-service
```

If Accessibility permission is tied to another signed binary, run the script with
that binary explicitly:

```bash
YABAI_BIN=/opt/homebrew/bin/yabai sh scripts/e2e-smoke.sh
```

It intentionally skips instead of failing when local preconditions are not safe:

- another yabai instance is already running for the current user
- Accessibility permission is not available
- Displays have separate Spaces is disabled
- `python3` is unavailable for JSON assertions

The smoke test does not exercise the scripting addition, Mission Control, space
dragging, or multi-display physical workflows. Those still need manual testing or
a dedicated self-hosted Mac with the required SIP, display, and permission state.
