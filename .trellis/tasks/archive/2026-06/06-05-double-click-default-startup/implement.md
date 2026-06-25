# Double-click Default Startup Implementation Plan

## Checklist

1. Update CLI parsing in `src/main.rs`.
   - Make the top-level command optional internally.
   - Preserve `--verbose` and `--strict` global flags.
   - Reuse `CommonArgs` and `RerunArgs` so no-argument/default visualizer live
     receives the same defaults as explicit `visualizer live`.

2. Add C++-style default live visualizer invocation.
   - `quanergy-client.exe` resolves to `visualizer live` with default args.
   - `quanergy-client.exe --host <SENSOR_IP>` resolves to `visualizer live
     --host <SENSOR_IP>`.
   - Explicit `visualizer live`, `visualizer replay`, `record`, and
     `dynamic-connection` continue to work.

3. Improve missing-host diagnostics.
   - Keep `require_host` as the central host guard.
   - Expand the config error message with examples for default and explicit
     visualizer live invocations.
   - Add a narrowly gated pause for no-argument launches after the missing-host
     message.

4. Add verification coverage.
   - Prefer integration-style command tests if the repo test setup can launch
     the compiled binary reliably.
   - Otherwise add focused unit coverage around the argument-resolution helper.
   - Cover no-argument default, top-level `--host`, explicit `visualizer live`,
     and explicit help behavior.

5. Run validation.
   - `rtk cargo fmt --all -- --check`
   - `rtk cargo clippy --all-targets --all-features -- -D warnings`
   - `rtk cargo test --all-targets --all-features`
   - `rtk just ci`

## Risky Files

- `src/main.rs`: CLI parsing and error behavior.

## Manual Checks

Run these after implementation:

```text
target\debug\quanergy-client.exe
target\debug\quanergy-client.exe --host 127.0.0.1
target\debug\quanergy-client.exe visualizer live
target\debug\quanergy-client.exe visualizer live --help
target\debug\quanergy-client.exe --help
```

Expected: bare launch follows the live visualizer path, missing host reports an
actionable configuration error, no-argument missing-host launch waits for Enter,
and help output remains available.

## Results

- `rtk just ci` passed.
- `target\debug\quanergy-client.exe` with piped Enter exits `1`, prints the
  actionable missing-host guidance, and shows `Press Enter to close this
  window...`.
- `target\debug\quanergy-client.exe --help` exits `0` and keeps root help.
- `target\debug\quanergy-client.exe visualizer live` exits `1` with the same
  missing-host guidance and does not pause.
- `target\debug\quanergy-client.exe visualizer live --help`, `record --help`,
  and `dynamic-connection --help` exit `0`.
- `target\debug\quanergy-client.exe --host 127.0.0.1` is accepted as the
  default visualizer live shorthand and reaches connection logic.
