# Capture Store Usage

The `capture-store` app writes station-frame point clouds for downstream tools.
It uses the same SDK pipeline as the visualizer, then applies a station
coordinate transform and persists standard PCD 0.7 files (`.pcd`) plus SQLite metadata.

## Live Capture

```powershell
rtk cargo run -p capture-store -- live --host 192.0.2.10 --output-dir capture-output
```

Default output:

```text
capture-store-output/
  capture.sqlite
  frames/
    <session-id>/
      frame_000000000001.pcd
```

Use `--database` when the SQLite database should live outside the output
directory:

```powershell
rtk cargo run -p capture-store -- live --host 192.0.2.10 --output-dir frames-out --database metadata/capture.sqlite
```

## Station Transform

The default transform is configured from scanner pose fields:

```powershell
rtk cargo run -p capture-store -- live --host 192.0.2.10 `
  --x-m 1.2 --y-m 0.5 --z-m 2.8 `
  --yaw-deg 0 --pitch-deg -12 --roll-deg 0
```

All pose fields default to `0`. The coordinate-frame label defaults to
`station` and can be changed with `--coord-frame`.

## Replay Storage

Replay converts a qraw capture into stored station-frame frames:

```powershell
rtk cargo run -p capture-store -- replay captures/session.qraw --output-dir replay-output
```

Replay applies calibration from the matching qraw sidecar when available. The
stored metadata keeps the qraw path so later tools can trace the frame source.

## Optional Raw Recording

Live storage does not continuously write qraw by default. Add `--record-raw`
when a debugging or calibration session also needs raw replay data:

```powershell
rtk cargo run -p capture-store -- live --host 192.0.2.10 --output-dir capture-output --record-raw
```

The qraw file is written under the output directory's `raw/` folder.

## Storage Queue

Live storage uses a bounded queue so slow disk or database writes cannot grow
memory without limit. The default capacity is `8` frames:

```powershell
rtk cargo run -p capture-store -- live --host 192.0.2.10 --storage-queue-capacity 16
```

If the queue is full, the app logs dropped-frame warnings instead of hiding the
backpressure.

## Shared Pipeline Flags

`capture-store` accepts the same core pipeline flags as `visualizer`:

| Flag | Purpose |
| --- | --- |
| `--settings-file`, `-s` | Load C++-style XML settings. |
| `--host` | Sensor host for live capture or calibration lookup. |
| `--strict` | Return parser errors instead of dropping bad packets. |
| `--return`, `-r` | Select return behavior. |
| `--calibrate` | Enable automatic encoder calibration. |
| `--manual-correct <AMPLITUDE> <PHASE>` | Use manual encoder correction. |
| `--min-distance` / `--max-distance` | Apply distance filtering. |
| `--min-cloud-size` / `--max-cloud-size` | Apply cloud-size filtering through pipeline config. |

## What This App Does Not Do

`capture-store` does not compute tamping-hammer ROI segmentation or height
statistics. Those are later measurement tools that should read the stored
station-frame clouds.
