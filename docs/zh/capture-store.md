# Capture Store 使用说明

`capture-store` 应用用于写入供下游工具使用的站点坐标系点云。它使用与 visualizer 相同的
SDK pipeline，然后应用站点坐标变换，并持久化 `.qpcd` 文件和 SQLite 元数据。

## 实时采集

```powershell
rtk cargo run -p capture-store -- live --host 192.0.2.10 --output-dir capture-output
```

默认输出：

```text
capture-store-output/
  capture.sqlite
  frames/
    <session-id>/
      frame_000000000001.qpcd
```

如果希望 SQLite 数据库放在 output directory 外部，使用 `--database`：

```powershell
rtk cargo run -p capture-store -- live --host 192.0.2.10 --output-dir frames-out --database metadata/capture.sqlite
```

## 站点坐标变换

默认变换通过 scanner pose 字段配置：

```powershell
rtk cargo run -p capture-store -- live --host 192.0.2.10 `
  --x-m 1.2 --y-m 0.5 --z-m 2.8 `
  --yaw-deg 0 --pitch-deg -12 --roll-deg 0
```

所有 pose 字段默认值都是 `0`。coordinate-frame 标签默认是 `station`，可以用
`--coord-frame` 修改。

## 回放存储

回放可以把 qraw 采集转换为已存储的站点坐标系帧：

```powershell
rtk cargo run -p capture-store -- replay captures/session.qraw --output-dir replay-output
```

如果 qraw sidecar 可用，回放会应用其中的标定信息。写入的元数据会保留 qraw 路径，
便于后续追踪 frame 来源。

## 可选原始录制

实时存储默认不会持续写 qraw。调试或标定会话需要原始回放数据时，添加 `--record-raw`：

```powershell
rtk cargo run -p capture-store -- live --host 192.0.2.10 --output-dir capture-output --record-raw
```

qraw 文件会写入 output directory 下的 `raw/` 目录。

## 存储队列

实时存储使用有界队列，避免慢磁盘或慢数据库写入导致内存无限增长。默认容量是 `8` 帧：

```powershell
rtk cargo run -p capture-store -- live --host 192.0.2.10 --storage-queue-capacity 16
```

如果队列已满，应用会记录 dropped-frame warning，而不是隐藏 backpressure。

## 共享 Pipeline 参数

`capture-store` 接受与 `visualizer` 相同的核心 pipeline 参数：

| 参数 | 作用 |
| --- | --- |
| `--settings-file`, `-s` | 加载 C++ 风格 XML settings。 |
| `--host` | 实时采集或标定读取使用的 sensor host。 |
| `--strict` | 返回 parser error，而不是丢弃坏包。 |
| `--return`, `-r` | 选择 return behavior。 |
| `--calibrate` | 启用自动 encoder calibration。 |
| `--manual-correct <AMPLITUDE> <PHASE>` | 使用手动 encoder correction。 |
| `--min-distance` / `--max-distance` | 应用距离过滤。 |
| `--min-cloud-size` / `--max-cloud-size` | 通过 pipeline config 应用 cloud-size 过滤。 |

## 这个应用不做什么

`capture-store` 不计算捣固锤 ROI 分割或高度统计。这些属于后续测量工具，
应读取已经存储的站点坐标系点云。
