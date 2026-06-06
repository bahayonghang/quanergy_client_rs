# Visualizer Usage

The `visualizer` app is the Rerun-based viewer for live sensor data and qraw
replay. It depends on `quanergy-client` for capture, parsing, calibration, and
frame production.

## Live Visualization

```powershell
rtk cargo run -p visualizer -- live --host 192.0.2.10
```

The top-level command also defaults to `live` when normal options are provided:

```powershell
rtk cargo run -p visualizer -- --host 192.0.2.10
```

Useful live options:

```powershell
rtk cargo run -p visualizer -- live --host 192.0.2.10 --calibrate
rtk cargo run -p visualizer -- live --host 192.0.2.10 --return all --min-distance 0.5 --max-distance 80
rtk cargo run -p visualizer -- live --settings-file ref/quanergy_client/settings/client.xml
```

## Rerun Output

By default, the app spawns a Rerun viewer. It can also connect to an existing
Rerun server or save an `.rrd` file:

```powershell
rtk cargo run -p visualizer -- live --host 192.0.2.10 --rerun-connect 127.0.0.1:9876
rtk cargo run -p visualizer -- replay sample.qraw --rerun-save sample.rrd
```

Use `--visualizer-max-points` to cap how many points are logged per frame. The
current default is `300000`.

## Replay

Replay consumes a `.qraw` capture. If a matching `.qraw.toml` sidecar exists,
the app uses its vertical angles and manual encoder correction values.

```powershell
rtk cargo run -p visualizer -- replay sample.qraw
rtk cargo run -p visualizer -- replay sample.qraw --realtime
```

`--realtime` sleeps between packets according to the recorded arrival deltas.

## Recording

Use `record` for raw packet capture without visualization:

```powershell
rtk cargo run -p visualizer -- record --host 192.0.2.10 captures/session.qraw
```

Use `live --record` when you want visualization and raw capture at the same
time:

```powershell
rtk cargo run -p visualizer -- live --host 192.0.2.10 --record captures/session.qraw
```

Both paths write a `.qraw.toml` sidecar next to the qraw file. The sidecar
contains deviceInfo-derived calibration metadata when available.

## Shared Pipeline Flags

These flags are shared by live, replay, and record where the command supports
them:

| Flag | Purpose |
| --- | --- |
| `--settings-file`, `-s` | Load C++-style XML settings. |
| `--host` | Sensor host for live capture or calibration lookup. |
| `--strict` | Return parser errors instead of dropping bad packets. |
| `--frame`, `-f` | Override frame id. |
| `--return`, `-r` | Select return behavior. |
| `--calibrate` | Enable automatic encoder calibration. |
| `--frame-rate` | Set frame rate for automatic calibration. |
| `--manual-correct <AMPLITUDE> <PHASE>` | Use manual encoder correction. |
| `--min-distance` / `--max-distance` | Apply distance filtering. |
| `--min-cloud-size` / `--max-cloud-size` | Apply cloud-size filtering through pipeline config. |
