# Double-click Default Startup Design

## Current Behavior

`src/main.rs` defines a required top-level Clap subcommand. A bare
`quanergy-client.exe` invocation exits before any application logic runs.

The original C++ `visualizer.exe` has no subcommand layer. It parses visualizer
options directly, loads a settings file if provided, checks host availability,
then initializes `SensorClient`, `SensorPipeline`, and `VisualizerModule`.

The important compatibility target is defaulting to the visualizer executable
path. C++ does not provide a default host.

## Chosen Approach

Make the Rust top-level command optional internally, and resolve a missing
command to `Command::Visualizer(VisualizerSubcommand::Live(default_args))`.

Keep explicit commands unchanged:

- `quanergy-client.exe visualizer live ...`
- `quanergy-client.exe visualizer replay ...`
- `quanergy-client.exe record ...`
- `quanergy-client.exe dynamic-connection ...`

Support a C++-style shorthand for the default path by allowing common live
visualizer flags at the top level when no explicit subcommand is supplied. The
target form is:

```text
quanergy-client.exe --host <SENSOR_IP>
```

This maps to:

```text
quanergy-client.exe visualizer live --host <SENSOR_IP>
```

The implementation should prefer reusing `CommonArgs` and `RerunArgs` rather
than duplicating option parsing.

## Error Behavior

The no-host error remains a configuration error because connecting to a sensor
without a host is impossible and C++ also rejects empty host. The message should
be actionable:

```text
Error: configuration error: no host provided

Run one of:
  quanergy-client.exe --host <SENSOR_IP>
  quanergy-client.exe visualizer live --host <SENSOR_IP>
  quanergy-client.exe visualizer live --settings-file <client.xml>
```

For no-argument launches that fail because no host is configured, pause for
Enter after printing the message. This pause is intentionally scoped to the
double-click-like default launch path and should not affect explicit CLI
commands such as `visualizer live`, `record`, or `dynamic-connection`.

## Boundaries

- CLI parsing stays in `src/main.rs`.
- `build_config`, `run_visualizer_live`, and library modules remain the runtime
  behavior owners.
- No parser, calibration, replay, recording, storage, or visualization sink
  changes are required.

## Trade-offs

Rejected: hard-code a default sensor IP. The C++ reference explicitly says no
default host makes sense, and a wrong default would produce confusing connection
failures.

Rejected for this task: separate `visualizer.exe` compatibility wrapper. It
matches C++ names more directly, but the prior milestone chose a primary unified
CLI and left wrappers as optional future work. The no-argument default solves
the user's immediate double-click issue with less churn.

Decision: pause after a no-argument missing-host error. This helps double-click
users read the message. Keep it narrowly gated so terminal and scripted explicit
commands do not gain a surprise prompt.

## Rollback

Revert the CLI default-command handling in `src/main.rs`. Library behavior and
wire/protocol behavior should be unaffected.
