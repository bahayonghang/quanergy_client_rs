# Quanergy Client RS

`quanergy_client_rs` 是对 Quanergy C++ client SDK 有用数据链路的 Rust
功能重写。当前仓库聚焦原 SDK 工作流：采集数据包、解析已支持的数据包类型、
应用标定和过滤、把 HVDIR 点转换为 XYZIR 帧、实时或离线可视化，并把站点坐标
系下的点云持久化，供后续测量程序使用。

第一里程碑面向 Windows `x86_64-pc-windows-msvc`。C++ ABI 兼容、PCL、
Boost、VTK，以及 Linux/macOS 优先设计都不是当前里程碑目标。

## 工作区

| 路径 | 作用 |
| --- | --- |
| `crates/quanergy-client` | 可复用 SDK 库，负责采集、协议、标定、帧、回放、坐标变换和存储。 |
| `apps/visualizer` | 基于 Rerun 的实时/回放可视化程序和 qraw 录制程序。 |
| `apps/capture-store` | 站点坐标系点云采集与回放存储程序，写入 `.qpcd` 帧和 SQLite 元数据。 |
| `ref/quanergy_client` | 本地 C++ SDK 参考实现，用于协议和行为对齐。 |

## 常用命令

```powershell
rtk just ci
rtk cargo run -p visualizer -- live --host 192.0.2.10
rtk cargo run -p visualizer -- replay sample.qraw --rerun-save sample.rrd
rtk cargo run -p capture-store -- live --host 192.0.2.10 --output-dir capture-output
rtk cargo run -p capture-store -- replay sample.qraw --output-dir replay-output
```

## 文档导航

- [项目架构](./architecture.md)：工作区边界，以及当前范围与后续范围。
- [实现细节](./implementation.md)：数据包、标定、帧、坐标变换和存储链路。
- [Visualizer 使用说明](./visualizer.md)：实时可视化、回放、录制和 Rerun 输出。
- [Capture Store 使用说明](./capture-store.md)：站点坐标系采集、回放存储、`.qpcd` 和 SQLite 元数据。
