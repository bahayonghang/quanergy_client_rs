# Visualizer 使用说明

`visualizer` 应用是基于 Rerun 的实时传感器数据和 qraw 回放查看器。它依赖
`quanergy-client` 完成采集、解析、标定和帧生产。

## 实时可视化

```powershell
rtk cargo run -p visualizer -- live --host 192.0.2.10
```

当提供普通选项时，顶层命令也会默认进入 `live`：

```powershell
rtk cargo run -p visualizer -- --host 192.0.2.10
```

常用实时选项：

```powershell
rtk cargo run -p visualizer -- live --host 192.0.2.10 --calibrate
rtk cargo run -p visualizer -- live --host 192.0.2.10 --return all --min-distance 0.5 --max-distance 80
rtk cargo run -p visualizer -- live --settings-file ref/quanergy_client/settings/client.xml
```

## Rerun 输出

默认情况下，应用会启动 Rerun viewer。也可以连接到已有 Rerun server，或保存 `.rrd` 文件：

```powershell
rtk cargo run -p visualizer -- live --host 192.0.2.10 --rerun-connect 127.0.0.1:9876
rtk cargo run -p visualizer -- replay sample.qraw --rerun-save sample.rrd
```

使用 `--visualizer-max-points` 限制每帧写入 Rerun 的点数。当前默认值是 `300000`。

## 回放

回放读取 `.qraw` 采集文件。如果同名 `.qraw.toml` sidecar 存在，应用会使用其中的
vertical angles 和手动 encoder correction 值。

```powershell
rtk cargo run -p visualizer -- replay sample.qraw
rtk cargo run -p visualizer -- replay sample.qraw --realtime
```

`--realtime` 会按照录制时的 packet arrival delta 进行 sleep。

## 录制

使用 `record` 可以只做原始 packet 采集，不启动可视化：

```powershell
rtk cargo run -p visualizer -- record --host 192.0.2.10 captures/session.qraw
```

需要同时可视化和原始采集时，使用 `live --record`：

```powershell
rtk cargo run -p visualizer -- live --host 192.0.2.10 --record captures/session.qraw
```

两种路径都会在 qraw 文件旁写入 `.qraw.toml` sidecar。deviceInfo 可用时，sidecar
会包含由 deviceInfo 得到的标定元数据。

## 共享 Pipeline 参数

这些参数由 live、replay 和 record 中支持它们的命令共享：

| 参数 | 作用 |
| --- | --- |
| `--settings-file`, `-s` | 加载 C++ 风格 XML settings。 |
| `--host` | 实时采集或标定读取使用的 sensor host。 |
| `--strict` | 返回 parser error，而不是丢弃坏包。 |
| `--frame`, `-f` | 覆盖 frame id。 |
| `--return`, `-r` | 选择 return behavior。 |
| `--calibrate` | 启用自动 encoder calibration。 |
| `--frame-rate` | 为自动标定设置 frame rate。 |
| `--manual-correct <AMPLITUDE> <PHASE>` | 使用手动 encoder correction。 |
| `--min-distance` / `--max-distance` | 应用距离过滤。 |
| `--min-cloud-size` / `--max-cloud-size` | 通过 pipeline config 应用 cloud-size 过滤。 |
