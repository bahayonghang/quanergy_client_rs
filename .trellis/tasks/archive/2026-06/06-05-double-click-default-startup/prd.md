# Double-click default startup parity

## Goal

Make the Rust `quanergy-client.exe` behave more like the original C++ example
applications when launched without CLI arguments, especially from Windows
double-click. The user-facing value is that opening the executable should follow
the visualizer path by default instead of immediately exiting with a Clap
missing-subcommand usage error.

Implementation was approved by the user and completed in `src/main.rs`.

## Confirmed Facts

- Current Rust binary is a single CLI with required subcommands:
  `visualizer`, `record`, and `dynamic-connection`.
- Running `target\debug\quanergy-client.exe` with no arguments currently prints
  top-level usage and exits with code `2`.
- Running `target\debug\quanergy-client.exe visualizer` with no nested
  subcommand also prints usage and exits with code `2`.
- Running `target\debug\quanergy-client.exe visualizer live` reaches the live
  visualizer path, then fails with `configuration error: no host provided` when
  no host is configured.
- The C++ `apps/visualizer.cpp` executable has no subcommand layer. With normal
  options parsing complete, it proceeds into the visualizer initialization path.
- C++ `SensorPipelineSettings::host` has no default host; the header comment
  says no default value makes sense. C++ examples document
  `visualizer.exe --host <IP Address of Sensor>`.
- The C++ settings sample keeps `<host></host>` empty and expects users to set
  host through CLI or settings.
- Prior Rust rewrite requirements explicitly chose one primary CLI with
  subcommands such as `visualizer live`, `visualizer replay`, `record`, and
  `dynamic-connection`; compatibility wrapper binaries may be added later if
  needed.

## Requirements

- Preserve the existing explicit CLI commands and flags.
- Treat no-argument root launch as the default visualizer live command path,
  matching the spirit of the C++ `visualizer.exe` example while keeping Rust's
  unified CLI.
- Do not invent a default sensor IP address.
- Keep settings-file and command-line precedence unchanged.
- Improve no-host feedback so double-click users can see the required next
  action instead of a transient top-level subcommand error.
- When launched with no arguments and no host is available, pause for Enter
  after printing the actionable missing-host message so a double-clicked
  console window does not disappear before the user can read it.
- Keep CLI and protocol/pipeline boundaries intact; do not move visualizer,
  capture, parsing, or calibration logic into the CLI entry point.

## Acceptance Criteria

- [x] `target\debug\quanergy-client.exe` no longer fails with a Clap
      missing-subcommand error.
- [x] No-argument launch maps to the same logic as
      `quanergy-client.exe visualizer live`.
- [x] If no host is configured, the program reports the missing host with an
      actionable example such as `quanergy-client.exe --host <SENSOR_IP>` or
      `quanergy-client.exe visualizer live --host <SENSOR_IP>`.
- [x] A no-argument missing-host launch waits for Enter before exiting.
- [x] `quanergy-client.exe --help`,
      `quanergy-client.exe visualizer live --help`,
      `quanergy-client.exe visualizer replay ...`, `record`, and
      `dynamic-connection` remain valid.
- [x] CLI tests or equivalent command checks cover no-argument default launch
      and missing-host behavior.
- [x] `just ci` passes.

## Out of Scope

- Choosing or hard-coding a real sensor host/IP.
- Starting live recording by default.
- Changing parser, calibration, pipeline, replay, storage, or Rerun frame
  rendering behavior.
- Adding station-frame transforms, storage formats, ROI processing, or
  tamping-station measurement.
- Replacing the unified Rust CLI with separate application-only binaries in
  this task.

## Decisions

- Confirmed: no-argument launch with no configured host should pause for Enter
  after printing the missing-host guidance.

## Notes

- Keep `prd.md` focused on requirements, constraints, and acceptance criteria.
- Lightweight tasks can remain PRD-only.
- For complex tasks, add `design.md` for technical design and `implement.md` for execution planning before `task.py start`.
