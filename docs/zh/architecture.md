# 项目架构

仓库把 SDK 逻辑放在可复用库中，把应用行为放在工作区应用中。CLI 应用应保持为
库模块的薄封装；协议、标定、pipeline、坐标变换和存储逻辑属于
`crates/quanergy-client`。

## 工作区边界

```text
Quanergy 传感器或 qraw 文件
  -> quanergy-client SDK 库
  -> visualizer 应用或 capture-store 应用
```

| 包 | 职责 | 应用专属依赖 |
| --- | --- | --- |
| `quanergy-client` | TCP 采集、数据包协议、deviceInfo、标定、过滤、帧组装、回放、坐标变换、qpcd、SQLite 元数据。 | 不包含 visualizer 路径的 UI 依赖。 |
| `visualizer` | CLI 解析、实时/回放编排、qraw 录制、Rerun sink 输出。 | `rerun`、`clap`、tracing 初始化。 |
| `capture-store` | CLI 解析、站点坐标变换配置、实时/回放存储编排、有界存储队列。 | `clap`、tracing 初始化。 |

## SDK 模块

| 模块 | 作用 |
| --- | --- |
| `net` | 连接 Quanergy TCP `4141` 数据流，并从 HTTP `7780` 获取 deviceInfo XML。 |
| `protocol` | 解析 20 字节数据包头，校验签名 `0x75bd7e97`，并定义 `RawPacket`、return selection、角度查找表和默认垂直角。 |
| `config` | 把 SDK 设置和 deviceInfo 值读入 `PipelineConfig`。 |
| `calibration` | 应用手动或自动 encoder correction。 |
| `filters` | 在 XYZIR 输出前应用距离过滤和 ring/intensity 过滤。 |
| `pipeline` | 分发 packet parser，组装帧，应用标定和过滤，并输出 `Frame<PointXyzir>`。 |
| `cloud` | 定义 `Frame`、`PointHvdir` 和 `PointXyzir`，包含 HVDIR 到 XYZIR 转换。 |
| `replay` | 读写 `.qraw` 数据包和 `.qraw.toml` 标定 sidecar。 |
| `transform` | 通过可复用坐标变换接口应用站点坐标变换。 |
| `storage` | 读写标准 PCD 0.7 文件和 SQLite 元数据。旧 `.qpcd` 只读兼容。 |

## 当前范围

当前已经实现的第一里程碑和近期存储能力包括：

- TCP 数据包采集和 qraw 回放。
- 数据包头校验，以及 `0x00`、`0x01`、`0x04`、`0x06` parser 分发。
- deviceInfo 解析：model、vertical angles、encoder amplitude、encoder phase。
- 基于 sweep/wrap 的 M-Series 帧组装，而不是把每个 TCP packet 当成一帧。
- 手动和自动 encoder calibration。
- HVDIR 到 XYZIR 转换和可复用帧类型。
- `visualizer` 应用中的 Rerun 可视化。
- SDK 和 `capture-store` 应用中的站点坐标变换、PCD 0.7 二进制帧文件和 SQLite 元数据。旧 `.qpcd` 仅作迁移用只读兼容。

## 后续业务工作

捣固站 ROI 分割、32 个捣固锤分组、高度统计和测量结果表都属于后续业务扩展。
它们应消费当前存储链路产出的站点坐标系点云，而不是改变采集、解析或可视化架构。
