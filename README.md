# Quanergy Client RS

`quanergy_client_rs` 是对 Quanergy C++ client SDK 有用数据链路的 Rust 功能重写。当前目标不是保持 C++ ABI 兼容，也不是绑定 PCL/Boost/VTK，而是把原 SDK 中对现场可用的采集、解析、标定、转换、可视化和存储能力做成可复用的 Rust 库，并由应用程序薄封装调用。

第一里程碑优先面向 Windows `x86_64-pc-windows-msvc`，重点支持实时 visualizer 路径：从 Quanergy TCP 数据流读取原始 packet，解析为点云帧，转换为 XYZIR，并用 Rerun 实时显示。

## 工作区结构

```text
quanergy_client_rs/
├── crates/
│   └── quanergy-client/      # 可复用 Rust SDK 库
├── apps/
│   ├── visualizer/           # 实时/回放点云可视化与 qraw 录制
│   └── capture-store/        # 站点坐标系点云采集、回放和持久化
├── docs/                     # VitePress 文档站内容
├── ref/                      # C++ SDK 参考实现与重写计划
├── Cargo.toml                # Rust workspace
└── justfile                  # 常用构建、测试、文档命令
```

Workspace 当前包含 3 个 Rust 包：

| 包 | 路径 | 类型 | 作用 |
| --- | --- | --- | --- |
| `quanergy-client` | `crates/quanergy-client` | library | SDK 核心库，提供协议、采集、解析、标定、转换、回放和存储 API。 |
| `visualizer` | `apps/visualizer` | library + binary | 基于 `quanergy-client` 和 Rerun 的实时/离线点云查看器，也可录制 `.qraw`。 |
| `capture-store` | `apps/capture-store` | binary | 把实时或回放点云转换到站点坐标系，并写入 `.qpcd` 帧和 SQLite 元数据。 |

## `quanergy-client` SDK 库结构

库入口在 `crates/quanergy-client/src/lib.rs`。公共 API 使用符合 Rust 习惯的命名，同时保留了便于从 C++ SDK 迁移的别名：

- `PointHVDIR` → `cloud::PointHvdir`
- `PointXYZIR` → `cloud::PointXyzir`

主要模块如下：

| 模块 | 主要能力 |
| --- | --- |
| `protocol` | Quanergy packet header、原始 packet、常量和基础协议工具。支持固定 20 字节 header、signature `0x75bd7e97`、默认 TCP 端口 `4141`、deviceInfo 端口 `7780`。 |
| `net` | 实时 TCP packet source。连接 sensor，读取 header/body，并封装为 `RawPacket`；也提供 `/PSIA/System/deviceInfo` XML 获取函数。 |
| `config` | Pipeline 配置、C++ 风格 XML settings 读取、deviceInfo 解析和 sensor model/vertical angles/encoder correction 应用。 |
| `pipeline` | `SensorPipeline` 主处理链路。负责按 packet type 分派 parser，应用 encoder correction、距离过滤、ring/intensity 过滤，并输出 `Frame<PointXyzir>`。 |
| `calibration` | M-Series encoder correction 的手动参数、自动标定计算和应用逻辑。 |
| `cloud` | 点云基础类型：`Frame<T>`、`PointHvdir`、`PointXyzir`，以及 HVDIR → XYZIR 转换。 |
| `filters` | 距离过滤、ring/intensity 过滤等 pipeline 过滤能力。 |
| `replay` | `.qraw` 读写、录制时间间隔保存、`.qraw.toml` sidecar 元数据。 |
| `transform` | 站点坐标系变换。当前提供 yaw/pitch/roll pose 到 4x4 transform 的能力。 |
| `storage` | `.qpcd` 点云帧读写和 SQLite 元数据存储。 |
| `error` | 统一的 `QuanergyError` 和 `Result<T>`。 |

### 当前核心数据流

```text
Quanergy sensor TCP:4141
    ↓
net::TcpPacketSource
    ↓
protocol::RawPacket / PacketHeader
    ↓
pipeline::SensorPipeline
    ↓
packet parser: 0x00 / 0x01 / 0x04 / 0x06
    ↓
Frame<PointHvdir>
    ↓
encoder correction + filters
    ↓
Frame<PointXyzir>
    ↓
visualizer 或 capture-store
```

### 已覆盖的 packet type

`pipeline` 中的 parser dispatch 当前覆盖原 SDK 优先路径需要的 packet type：

| Packet type | 名称/用途 |
| --- | --- |
| `0x00` | M-Series packet。 |
| `0x01` | HVDIR list packet。 |
| `0x04` | Reduced-bandwidth M-Series packet。 |
| `0x06` | M1 packet。 |

### SDK 使用示例

```rust
use quanergy_client::{
    config::PipelineConfig,
    net::TcpPacketSource,
    pipeline::SensorPipeline,
    Result,
};

fn main() -> Result<()> {
    let host = "192.0.2.10";
    let mut source = TcpPacketSource::connect(host)?;
    let mut pipeline = SensorPipeline::new(PipelineConfig {
        host: host.to_owned(),
        ..PipelineConfig::default()
    })?;

    loop {
        let packet = source.next_packet()?;
        for frame in pipeline.process_raw(&packet)? {
            println!("frame {}: {} points", frame.sequence, frame.points.len());
        }
    }
}
```

## 应用说明

### `apps/visualizer`

`visualizer` 是实时和回放点云查看器：

- 实时连接 sensor，读取 TCP `4141` 原始 packet。
- 自动尝试读取 deviceInfo，用于补全 model、vertical angles 和 encoder 参数。
- 通过 `SensorPipeline` 输出 XYZIR 帧。
- 使用 Rerun 显示点云，支持 spawn viewer、连接已有 viewer 或保存 `.rrd`。
- 可录制 `.qraw`，并在旁边写入 `.qraw.toml` sidecar。

常用命令：

```powershell
cargo run -p visualizer -- live --host 192.0.2.10
cargo run -p visualizer -- --host 192.0.2.10
cargo run -p visualizer -- live --host 192.0.2.10 --record captures/session.qraw
cargo run -p visualizer -- record --host 192.0.2.10 captures/session.qraw
cargo run -p visualizer -- replay captures/session.qraw --realtime
cargo run -p visualizer -- replay captures/session.qraw --rerun-save captures/session.rrd
```

共享 pipeline 参数包括：

- `--settings-file`, `-s`：加载 C++ 风格 XML settings。
- `--host`：sensor host。
- `--strict`：遇到 parser error 时直接返回错误，而不是丢弃坏包。
- `--frame`, `-f`：覆盖 frame id。
- `--return`, `-r`：选择 return，例如单个 return 或 `all`。
- `--calibrate`：启用自动 encoder calibration。
- `--frame-rate`：设置自动标定使用的 frame rate。
- `--manual-correct <AMPLITUDE> <PHASE>`：使用手动 encoder correction。
- `--min-distance` / `--max-distance`：距离过滤。
- `--min-cloud-size` / `--max-cloud-size`：cloud size 过滤参数。

### `apps/capture-store`

`capture-store` 用于把 SDK pipeline 输出的点云转换到站点坐标系并持久化：

- 支持实时采集和 `.qraw` 回放。
- 应用 yaw/pitch/roll + 平移配置得到站点坐标系点云。
- 每帧写入一个 `.qpcd` 二进制点云文件。
- SQLite 保存 session、frame metadata、坐标变换、标定快照和 qraw 来源。
- 实时模式使用有界存储队列，避免慢磁盘/数据库阻塞 packet ingestion。
- 可选 `--record-raw` 同步保存原始 `.qraw`。

常用命令：

```powershell
cargo run -p capture-store -- live --host 192.0.2.10 --output-dir capture-output
cargo run -p capture-store -- live --host 192.0.2.10 --output-dir capture-output --record-raw
cargo run -p capture-store -- replay captures/session.qraw --output-dir replay-output
```

站点坐标变换示例：

```powershell
cargo run -p capture-store -- live --host 192.0.2.10 `
  --x-m 1.2 --y-m 0.5 --z-m 2.8 `
  --yaw-deg 0 --pitch-deg -12 --roll-deg 0
```

默认输出结构：

```text
capture-store-output/
├── capture.sqlite
└── frames/
    └── <session-id>/
        └── frame_000000000001.qpcd
```

如需把数据库放到其他位置：

```powershell
cargo run -p capture-store -- live --host 192.0.2.10 \
  --output-dir frames-out \
  --database metadata/capture.sqlite
```

> `capture-store` 不负责 ROI 分割或捣固锤高度统计；这些属于后续测量工具，应读取已持久化的站点坐标系点云。

## 使用 justfile 构建两个应用

本仓库根目录提供 `justfile`。可以先查看可用任务：

```powershell
just --list
```

### Debug 构建

```powershell
just build
```

`just build` 当前等价于：

```powershell
cargo build
```

因为根 `Cargo.toml` 是 workspace，执行后会构建整个 workspace，包括：

- `apps/visualizer`
- `apps/capture-store`
- `crates/quanergy-client`

Windows debug 产物通常位于：

```text
target/debug/visualizer.exe
target/debug/capture-store.exe
```

### Release 构建

```powershell
just release
```

`just release` 当前等价于：

```powershell
cargo build --release
```

Windows release 产物通常位于：

```text
target/release/visualizer.exe
target/release/capture-store.exe
```

### 只构建某一个应用

`justfile` 目前没有单独的 `visualizer` 或 `capture-store` recipe。如果只想构建某一个应用，可以直接用 cargo package 参数：

```powershell
cargo build -p visualizer
cargo build -p capture-store
cargo build --release -p visualizer
cargo build --release -p capture-store
```

### 开发检查

常用检查命令：

```powershell
just check      # cargo check --all-targets --all-features
just test       # cargo test --all-targets --all-features
just clippy     # cargo clippy --all-targets --all-features -- -D warnings
just fmt-check  # cargo fmt --all -- --check
just ci         # fmt-check + clippy + test
```

## 更多文档

- `docs/zh/index.md`：中文文档入口。
- `docs/zh/architecture.md`：架构和范围说明。
- `docs/zh/implementation.md`：协议、pipeline、标定、存储等实现细节。
- `docs/zh/visualizer.md`：`visualizer` 使用说明。
- `docs/zh/capture-store.md`：`capture-store` 使用说明。
- `ref/refactor_plan.md`：Rust 重写技术方向和原 SDK 行为参考。
