# 实现细节

本页说明当前 Rust 数据链路，以及后续工作需要保持的契约。

## 数据包输入

实时采集从 `TcpPacketSource` 开始：

```text
sensor_ip:4141
  -> 20-byte PacketHeader
  -> signature check 0x75bd7e97
  -> packet body
  -> RawPacket
```

SDK 也支持 qraw 回放：

```text
.qraw + optional .qraw.toml sidecar
  -> QrawReader
  -> RawPacket
```

启用实时录制时，`.qraw` 会保留原始 packet bytes，sidecar 会在可用时记录标定元数据。
如果 deviceInfo 读取失败，sidecar 会记录标定不完整标记和错误原因。

## 标定输入

库可以从传感器读取标定元数据：

```text
http://<sensor-host>:7780/PSIA/System/deviceInfo
```

解析字段包括 sensor model、vertical angles、encoder amplitude 和 encoder phase。
settings 文件和 CLI 参数可以在现场需要时覆盖 pipeline 配置。

## Pipeline 流程

`SensorPipeline` 负责 parser 分发和帧处理：

```text
RawPacket
  -> parser dispatch for 0x00 / 0x01 / 0x04 / 0x06
  -> Frame<PointHvdir>
  -> optional automatic or manual encoder correction
  -> distance filter
  -> ring/intensity filter
  -> Frame<PointXyzir>
```

非 strict 模式下，坏包会计数并带 warning 丢弃。strict 模式下，parser error 会返回给调用方。

## 帧语义

帧通过 M-Series sweep/wrap 逻辑组装。除非调用方明确在做 packet-level debug，
否则 TCP packet 不会被当作完整扫描帧。这一点对 visualizer 对齐和存储质量都很重要。

`Frame<PointXyzir>` 保留：

- `frame_id`
- `stamp_micros`
- `sequence`
- organized cloud dimensions
- dense flag
- 点数据：`x`、`y`、`z`、`intensity`、`ring`

## 站点坐标变换

存储路径可以把 sensor-frame XYZIR 点转换为 station frame。默认面向应用的变换是
yaw/pitch/roll 位姿：

```text
x_m, y_m, z_m
yaw_deg, pitch_deg, roll_deg
```

SDK 暴露了坐标变换边界，因此后续标定算法可以替换默认位姿变换，而不需要重写采集或存储编排。

## 点云存储

生产存储使用每个完整帧一个 `.qpcd` 二进制文件。`.qpcd` 文件以 `QPCDv1` magic 开头，
后面是 JSON header 和重复的 20 字节 XYZIR 点记录：

```text
x: f32
y: f32
z: f32
intensity: f32
ring: u16
flags: u16
```

文件先写到临时路径，完成后再 rename 到最终 `.qpcd` 路径。

## SQLite 元数据

`SqliteStore` 当前创建两张表：

| 表 | 作用 |
| --- | --- |
| `capture_session` | session id、开始/结束时间、sensor host/model、SDK version、status 和 notes。 |
| `scan_frame` | frame sequence、timestamp、point count、coordinate frame、transform snapshot、calibration snapshot、`.qpcd` 路径、可选 qraw 路径和 status。 |

生产路径刻意避免逐点 SQL 行。下游工具应先从 SQLite 枚举 frame，再读取对应 `.qpcd` 文件。
