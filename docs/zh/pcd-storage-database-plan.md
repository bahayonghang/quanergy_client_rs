# PCD 点云存储迁移与数据库保存接口计划

> 状态：设计计划，尚未实施  
> 适用仓库：`quanergy_client_rs`  
> 目标平台：第一里程碑仍以 `x86_64-pc-windows-msvc` 为准  
> 本文档变更范围：**仅新增计划文档，不修改 Rust 实现、数据库 schema 或运行时行为**

## 1. 结论摘要

本计划采用以下明确决策：

1. 将 `capture-store` 的默认完整帧点云文件从私有 `.qpcd` 切换为标准 **PCD 0.7**。
2. 生产默认编码使用 `DATA binary`；`binary_compressed` 保留为可选编码，必须经过实时写入吞吐和队列丢帧基准后才能成为默认值；`ascii` 仅用于调试和互操作排查。
3. PCD 点字段固定为 `x y z intensity ring`，对应当前 `PointXyzir`，不再保留 `.qpcd` 中始终为 `0` 的预留 `flags` 字段。
4. PCD 只负责点数组、组织维度和 acquisition viewpoint。`sequence`、时间戳、坐标系名称、变换、标定和来源路径等帧级元数据继续由数据库负责。
5. 新增一个**只声明、不实现、不接线**的数据库保存接口：`ScanFrameMetadataStore::save_scan_frame_metadata`。不得用 `todo!()`、`unimplemented!()` 或默认 panic 伪装成实现。
6. 当前已有的 `SqliteStore` 和 `insert_scan_frame` 不应被误判为“数据库能力尚不存在”。新的接口用于解除上层代码对 SQLite 具体类型的耦合；本任务不增加 PostgreSQL 等第二后端，也不实现该 trait。
7. 停止生成新的 `.qpcd`，但保留旧格式 reader 的迁移期兼容，避免已有采集数据立即不可读。
8. 数据库仍然只保存每帧元数据和结果，不保存逐点 SQL 行。

## 2. 当前实现基线

### 2.1 当前点云文件格式

`crates/quanergy-client/src/storage/qpcd.rs` 实现了私有 `QPCDv1`：

- magic：`QPCDv1\0\0`
- 预留标志：`u32`
- JSON header 长度：`u32`
- JSON header
- 重复的 20 字节点记录：
  - `x: f32`
  - `y: f32`
  - `z: f32`
  - `intensity: f32`
  - `ring: u16`
  - `flags: u16`，当前恒为 `0`

`QpcdHeader` 同时保存了点云文件属性和业务帧元数据：

- `format_version`
- `point_stride`
- `point_count`
- `stamp_micros`
- `sequence`
- `coord_frame`
- `frame_id`
- `width`
- `height`
- `is_dense`

这一设计便于项目内部 round-trip，但它不是通用点云交换格式，外部工具必须理解 Quanergy 私有 magic 和 JSON header。

### 2.2 当前数据库能力

`crates/quanergy-client/src/storage/sqlite.rs` 已经存在 SQLite 实现：

- `SqliteStore::open`
- `SqliteStore::init_schema`
- `SqliteStore::insert_capture_session`
- `SqliteStore::finish_capture_session`
- `SqliteStore::insert_scan_frame`
- session/frame 查询方法

当前表：

- `capture_session`
- `scan_frame`

当前 `scan_frame` 已保存：

- session 和 sequence
- timestamp
- sensor host/model
- packet type mask
- point count
- coordinate frame
- 4x4 transform 及 JSON snapshot
- calibration JSON
- cloud path
- qraw path
- status 和 created time

因此，本计划中的“创建保存数据库的方法”应理解为**新增后端无关的 API 边界**，而不是再复制一个 SQLite insert 方法。

### 2.3 当前持久化顺序

`apps/capture-store/src/main.rs` 中的 `FramePersister::persist_frame` 当前执行：

```text
station-frame Frame<PointXyzir>
  -> write_qpcd(temp + rename)
  -> 构造 NewScanFrame
  -> SqliteStore::insert_scan_frame
```

实时模式使用有界同步队列，存储线程不能反向阻塞 packet ingestion；队列满时会显式丢帧并记录 warning。PCD 迁移不得破坏这一约束。

### 2.4 当前需要修正的语义边界

当前 transform 保留输入 `Frame.frame_id`，默认通常是 `quanergy`，但 `capture-store` 写出的点已经位于 `station` 坐标系，并另有 `StorageContext.coord_frame`。迁移时必须区分：

- `source_frame_id`：输入 pipeline 的 frame 名称，例如 `quanergy`
- `coord_frame`：落盘点坐标实际所属坐标系，例如 `station`

不得把二者压缩成一个含糊字段，也不能假设 PCD header 能表达 ROS/PCL 风格 frame name。

## 3. 目标与非目标

### 3.1 目标

- 生成符合 PCD 0.7 header 和数据布局的 `.pcd` 文件。
- 不依赖 PCL、Boost、VTK 或 C++ ABI。
- 保留 XYZIR 数值精度和 `ring: u16`。
- 保留 organized cloud 的 `WIDTH`/`HEIGHT` 语义。
- 正确表达 station-frame 点云对应的 sensor acquisition viewpoint。
- 保持临时文件写入和最终 rename，避免把半写文件暴露给下游。
- 保持实时存储有界队列和显式 backpressure 行为。
- 为未来 SQLite/PostgreSQL 等后端预留统一的帧元数据保存方法，但本次不实现该抽象。
- 提供旧 `.qpcd` 的只读迁移路径。

### 3.2 非目标

- 不把每个点写成数据库一行。
- 不实现 PostgreSQL、PostGIS、pgPointcloud 或对象存储。
- 不实现 ROI 分割、捣固锤高度计算或测量结果表。
- 不改变 `.qraw` 录制、回放和 `.qraw.toml` sidecar 语义。
- 不在 PCD comment 中创建另一套私有元数据协议。
- 不在本计划文档提交中修改任何 Rust 源码。

## 4. PCD 文件契约

参考 PCL 官方 PCD 0.7 定义：header 为 ASCII，字段顺序固定，数据可为 `ascii`、`binary` 或 `binary_compressed`。

### 4.1 固定点 schema

第一版使用以下 schema：

| PCD field | SIZE | TYPE | COUNT | Rust 类型 | 含义 |
| --- | ---: | --- | ---: | --- | --- |
| `x` | 4 | `F` | 1 | `f32` | X 坐标，米 |
| `y` | 4 | `F` | 1 | `f32` | Y 坐标，米 |
| `z` | 4 | `F` | 1 | `f32` | Z 坐标，米 |
| `intensity` | 4 | `F` | 1 | `f32` | 当前 pipeline 的 intensity |
| `ring` | 2 | `U` | 1 | `u16` | 激光 ring id |

对应 header：

```text
# .PCD v0.7 - Point Cloud Data file format
VERSION 0.7
FIELDS x y z intensity ring
SIZE 4 4 4 4 2
TYPE F F F F U
COUNT 1 1 1 1 1
WIDTH <frame.width>
HEIGHT <frame.height>
VIEWPOINT <tx> <ty> <tz> <qw> <qx> <qy> <qz>
POINTS <frame.points.len()>
DATA binary
```

说明：

- `flags` 不写入 PCD，因为当前 `.qpcd` writer 对它始终写 `0`，`PointXyzir` 也没有该成员。
- `binary` 下每点有效 payload 为 18 字节。不得为了复刻 `.qpcd` 的 20 字节 stride 人为加入无意义 padding field。
- 字段顺序属于文件契约，测试必须精确验证。
- `ring` 必须保持无符号 16 位，不能收窄为 `u8`。

### 4.2 组织维度

写入前必须验证：

```text
width > 0
height > 0
width * height == points.len()
```

空点云是否允许应由公共 API 明确决定。建议第一版允许 `POINTS 0`，但统一写：

```text
WIDTH 0
HEIGHT 1
POINTS 0
```

对于非空且维度不一致的 frame，writer 应返回 `StorageFormat` 错误，不应静默改写为 unorganized cloud，因为静默修复会掩盖 pipeline/frame assembly 缺陷。需要非组织化时，调用方应显式调用 `refresh_unorganized_dims()`。

### 4.3 VIEWPOINT 语义

PCD `VIEWPOINT` 表示 acquisition origin 和方向，不是 coordinate-frame 名称。

对于 `capture-store` 写出的 station-frame 点：

- translation 使用 `T_station_sensor` 的平移部分；
- quaternion 使用 `T_station_sensor` 左上 3x3 旋转矩阵转换得到的 `qw qx qy qz`；
- 变换矩阵必须有限、近似正交，且旋转行列式接近 `+1`；
- 不能验证为刚体变换时应返回错误，不能悄悄写入错误 quaternion。

通用 PCD writer 不应自行猜测坐标变换。调用方通过 `PcdWriteOptions.viewpoint` 显式传入；没有 acquisition pose 的调用方可以显式选择 identity viewpoint。

### 4.4 编码决策

| 编码 | 用途 | 第一版策略 |
| --- | --- | --- |
| `binary` | 实时/回放生产存储 | 默认；最接近当前未压缩 `.qpcd` 的 CPU 开销 |
| `binary_compressed` | 节省磁盘、归档 | 支持时作为 opt-in；成为默认前必须基准测试 |
| `ascii` | 调试、小型互操作样本 | 允许显式选择，不用于生产默认值 |

`binary_compressed` 会进行字段重排和 LZF 压缩。它可能显著降低文件大小，但也会增加存储线程 CPU 时间；在当前有界队列架构下，未经基准直接设为默认可能增加 dropped frames。

## 5. 帧级元数据归属

标准 PCD 不原生保存 Quanergy 的完整帧身份。迁移后按下表分层：

| 当前 `.qpcd` 信息 | PCD | 数据库 | 说明 |
| --- | --- | --- | --- |
| `format_version` | `VERSION 0.7` | `cloud_format/version` | PCD 和数据库都可识别格式 |
| `point_stride` | 由 schema 推导 | 可选 | 不再存私有 stride |
| `point_count` | `POINTS` | `point_count` | 双方必须一致 |
| `width` / `height` | `WIDTH` / `HEIGHT` | 建议冗余保存 | DB 可不打开文件完成筛选 |
| `stamp_micros` | 否 | 是 | 数据库权威 |
| `sequence` | 文件名只作提示 | 是 | 数据库权威；文件名仍含 sequence |
| `coord_frame` | 否 | 是 | 数据库权威 |
| `frame_id` | 否 | `source_frame_id` | 避免与 DB 主键 `frame_id` 混淆 |
| `is_dense` | 否 | 是 | 不能只靠 PCD header 精确恢复 |
| transform | `VIEWPOINT` 只表达 pose | 完整 snapshot | 数据库保存 4x4 和 JSON |
| calibration | 否 | 是 | 数据库权威 |
| qraw source | 否 | 是 | 数据库权威 |

数据库丢失时，`.pcd` 仍可被通用工具打开并读取点；但无法完整恢复 session、timestamp、原始 frame 名称和标定上下文。该限制必须在文档中明确，不应通过非标准 header comment 隐藏。

## 6. PCD 库 API 设计

建议新增 `crates/quanergy-client/src/storage/pcd.rs`，公共 API 不直接暴露第三方 crate 类型。

以下签名是实施目标，不在本计划提交中创建：

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PcdEncoding {
    Ascii,
    Binary,
    BinaryCompressed,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PcdViewpoint {
    pub translation_m: [f32; 3],
    pub rotation_wxyz: [f32; 4],
}

#[derive(Debug, Clone, PartialEq)]
pub struct PcdWriteOptions {
    pub encoding: PcdEncoding,
    pub viewpoint: PcdViewpoint,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PcdFileInfo {
    pub point_count: u64,
    pub width: usize,
    pub height: usize,
    pub encoding: PcdEncoding,
    pub file_size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PcdCloud {
    pub width: usize,
    pub height: usize,
    pub viewpoint: PcdViewpoint,
    pub points: Vec<PointXyzir>,
}

pub fn write_pcd(
    path: impl AsRef<Path>,
    frame: &Frame<PointXyzir>,
    options: &PcdWriteOptions,
) -> Result<PcdFileInfo>;

pub fn read_pcd(path: impl AsRef<Path>) -> Result<PcdCloud>;
```

### 6.1 为什么 `read_pcd` 不直接返回完整 `Frame<PointXyzir>`

PCD 文件本身没有 `stamp_micros`、`sequence`、`source_frame_id`、`coord_frame` 和 `is_dense`。如果 reader 伪造这些字段，会把“未知”错误表示成“已知”。

正确路径是：

```text
ScanFrameRecord from database
  + PcdCloud from file
  -> validated Frame<PointXyzir> / downstream measurement input
```

后续可增加显式组合函数，但不能在纯 PCD reader 中制造业务元数据。

### 6.2 第三方 crate 选择

优先评估纯 Rust `pcd-rs`：

- 支持 PCD 0.7；
- 支持 dynamic/static schema；
- 支持 ASCII、binary、binary-compressed；
- 不引入 PCL/C++ ABI。

采用前必须通过以下 gate：

1. 在 workspace 的 Rust `1.82` MSRV 下编译；
2. 在 `x86_64-pc-windows-msvc` 下构建和测试；
3. `u16 ring`、NaN、organized dimensions 和三个 encoding round-trip 正确；
4. 产物可由独立 PCL/CloudCompare/PDAL 工具读取；
5. dependency tree 和许可证可接受；
6. writer 的 `finish()`、文件 seek 和错误行为满足临时文件写入流程。

如果任一关键 gate 失败，fallback 是在本仓库实现**最小 PCD 0.7 固定 XYZIR codec**，仍然生成标准 PCD，而不是继续维护 `.qpcd`。fallback 第一版可只实现 `binary`，压缩支持另开任务。

### 6.3 避免污染核心点类型

不建议直接在 `cloud::PointXyzir` 上派生第三方 `PcdSerialize/PcdDeserialize`，否则核心 cloud 模块会依赖具体存储实现。

建议在 `storage::pcd` 内部定义私有 adapter：

```rust
struct PcdPointXyzir {
    x: f32,
    y: f32,
    z: f32,
    intensity: f32,
    ring: u16,
}
```

通过无损 `From` 转换连接 `PointXyzir`，使未来替换 codec 不影响公共 cloud API。

## 7. 数据库保存方法：只声明，不实现

### 7.1 拟新增接口

建议新增文件：

```text
crates/quanergy-client/src/storage/repository.rs
```

拟声明：

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScanFrameId(pub i64);

pub trait ScanFrameMetadataStore: Send {
    /// 在点云文件已经完成并原子提交到最终路径后，保存一条帧级元数据。
    ///
    /// 合约：
    /// - 不保存逐点 payload；
    /// - `(session_id, sequence)` 必须唯一；
    /// - 重复保存返回错误，不隐式覆盖；
    /// - 成功时返回后端分配的稳定 frame id；
    /// - 方法本身同步执行，由现有 storage worker 隔离磁盘/数据库延迟。
    fn save_scan_frame_metadata(
        &mut self,
        frame: &NewScanFrame,
    ) -> Result<ScanFrameId>;
}
```

### 7.2 本任务明确禁止的内容

在“只创建方法、不实现”的工作项中：

- 不写 `impl ScanFrameMetadataStore for SqliteStore`；
- 不增加 PostgreSQL backend；
- 不把 `FramePersister` 改成依赖该 trait；
- 不调用该方法；
- 不提供返回固定值的假实现；
- 不使用 `todo!()`、`unimplemented!()` 或 panic 默认方法；
- 不删除现有 `SqliteStore::insert_scan_frame`。

这样 trait 是真实的编译期接口声明，但不会制造一个可被误调用的运行时陷阱。

### 7.3 为什么使用 `&mut self` 和同步 trait

- 当前 `capture-store` 已在独立 storage worker thread 中执行阻塞 I/O，不需要为接口引入 async runtime。
- `&mut self` 明确表达单 writer、顺序提交和未来 transaction 状态。
- `Send` 允许 store 被移动到 storage worker；不要求 `Sync`，符合 `rusqlite::Connection` 的使用方式。
- `ScanFrameId` newtype 避免上层代码直接绑定 SQLite 的 `last_insert_rowid` 命名。

### 7.4 后续实现关系

未来单独任务可以实现：

```text
SqliteStore::insert_scan_frame
  -> 被内部复用或重命名
  -> impl ScanFrameMetadataStore for SqliteStore
```

该实现不是本计划文档提交的内容，也不是“只声明方法”工作项的内容。

## 8. 数据库 metadata 演进计划

切换 PCD 后，为避免丢失 `.qpcd` header 中的业务元数据，建议扩展 `NewScanFrame` / `ScanFrameRecord`：

```text
source_frame_id
width
height
is_dense
cloud_format          # pcd / qpcd
cloud_format_version  # 0.7 / 1
cloud_encoding        # binary / binary_compressed / ascii / custom-binary
file_size_bytes
point_schema          # xyzir-v1
```

### 8.1 必须先引入 schema version

当前 `init_schema()` 只有 `CREATE TABLE IF NOT EXISTS`，无法给已有表可靠增加列。实施数据库变更前必须引入 `PRAGMA user_version` 或等价 migration table。

建议版本：

- `user_version = 0`：当前未版本化 schema；
- `user_version = 1`：增加 PCD/帧维度相关 metadata；
- 后续版本单向递增，每次 migration 在 transaction 内完成。

对旧行的回填策略：

- `cloud_format = 'qpcd'`
- `cloud_format_version = '1'`
- `cloud_encoding = 'custom-binary'`
- `point_schema = 'xyzir-v1'`
- `width/height/source_frame_id/is_dense/file_size_bytes` 初期允许 nullable；可通过打开旧 `.qpcd` 后离线回填

新 PCD 行在 Rust domain model 中必须提供完整值，即使迁移期数据库列暂时允许 nullable。

### 8.2 不在初始迁移中加入的字段

`sha256` 能提高完整性验证，但会增加实时写入 CPU/读盘成本。建议先通过 `file_size_bytes + header/point-count validation` 完成第一版；checksum 作为独立可配置任务评估。

## 9. 文件与数据库的一致性顺序

建议的未来 `persist_frame` 流程：

```text
1. 验证 Frame dimensions、点数和 transform/viewpoint
2. 构造 final path: frame_<sequence>.pcd
3. 检查 final path 不存在；重复 sequence 不允许覆盖
4. 写 frame_<sequence>.pcd.tmp
5. writer.finish()
6. flush / 必要时 sync
7. 重新读取最小 header，验证 schema、POINTS、WIDTH、HEIGHT
8. rename tmp -> final
9. 构造 NewScanFrame，cloud_path 指向 final .pcd
10. 调用当前 SQLite insert；未来才切换到 save_scan_frame_metadata trait
11. 成功后记录 frame committed
```

### 9.1 错误处理矩阵

| 失败点 | 数据库状态 | 文件处理 | 期望行为 |
| --- | --- | --- | --- |
| validation 失败 | 无记录 | 不创建文件 | 返回明确错误 |
| temp 写入失败 | 无记录 | 删除 temp，best effort | 不暴露 final |
| `finish()`/flush 失败 | 无记录 | 删除 temp | 返回 storage error |
| header 自检失败 | 无记录 | 删除 temp | 禁止提交损坏文件 |
| rename 失败 | 无记录 | 保留或删除 temp并记录路径 | 不写 DB |
| DB insert 失败 | 无记录 | 删除刚提交 final，best effort | 避免普通 orphan |
| 进程在 rename 后、DB 前崩溃 | 无记录 | 可能留下 orphan | 后续 reconciliation 任务处理 |

文件系统和数据库无法组成真正的跨资源原子 transaction。第一版至少要保证“数据库不会指向未完成临时文件”，并提供可检测的 orphan 命名规则。启动时扫描 orphan/临时文件属于后续增强项。

### 9.2 不覆盖已有 final 文件

当前 `.qpcd` 的 Windows fallback 会在 rename 冲突时删除已有目标再替换。迁移后建议改为**拒绝覆盖**：

- 数据库已有 `(session_id, sequence)` 唯一约束；
- 文件层也应保持相同语义；
- 覆盖会在 DB insert 最终失败时破坏原有有效文件。

## 10. 旧 `.qpcd` 兼容与迁移

### 10.1 兼容策略

- 新版本不再调用 `write_qpcd`。
- 将旧实现移动或重命名为 `storage::legacy_qpcd`。
- 保留 `read_qpcd`，至少覆盖一个迁移周期。
- 公共 API 对 legacy reader 标记 deprecated，并说明只用于旧数据转换。
- 数据库通过 `cloud_format` 或扩展名选择 reader，不能把 `.qpcd` 当 PCD 解析。

### 10.2 可选转换工具

后续可增加独立命令，而不是塞进实时路径：

```text
capture-store migrate-qpcd \
  --database capture.sqlite \
  --output-dir migrated
```

每帧转换流程：

```text
read legacy qpcd
  -> write and validate pcd
  -> transactionally update cloud_path/format metadata
  -> 可配置保留或删除原 qpcd
```

转换工具不属于初始 PCD writer 切换的必要条件，但旧 reader 必须先保留。

## 11. 预期文件变更清单

以下是后续实施时的文件级影响，不在本计划提交中执行。

### 11.1 SDK 库

| 文件 | 计划变更 |
| --- | --- |
| `crates/quanergy-client/Cargo.toml` | 经 gate 后增加 `pcd-rs`；若失败则不增加依赖，使用内部最小 codec |
| `crates/quanergy-client/src/storage/pcd.rs` | 新增标准 PCD reader/writer、options、file info 和 adapter |
| `crates/quanergy-client/src/storage/qpcd.rs` | 迁移为 legacy reader；停止新写入 |
| `crates/quanergy-client/src/storage/mod.rs` | 导出 PCD API；对 legacy QPCD API 降级/弃用 |
| `crates/quanergy-client/src/storage/metadata.rs` | 增加 PCD 和帧维度 metadata 字段 |
| `crates/quanergy-client/src/storage/repository.rs` | 只声明 `ScanFrameMetadataStore::save_scan_frame_metadata`，不实现 |
| `crates/quanergy-client/src/storage/sqlite.rs` | 后续独立任务增加 migration 和 trait 实现；声明-only 工作项不修改此文件 |
| `crates/quanergy-client/src/storage/tests.rs` | 用 PCD round-trip、schema、legacy、DB path 测试替换/扩展 |
| `crates/quanergy-client/src/error.rs` | 将 codec 错误稳定映射为 storage error，不向公共 API 泄漏第三方错误类型 |

### 11.2 应用

| 文件 | 计划变更 |
| --- | --- |
| `apps/capture-store/src/main.rs` | `write_qpcd` -> `write_pcd`；`.qpcd` -> `.pcd`；变量和日志改名；传入 viewpoint/options |
| `apps/capture-store/Cargo.toml` | 原则上不直接依赖 `pcd-rs`，由 SDK 封装 |

可选 CLI：

```text
--pcd-encoding binary|binary-compressed|ascii
```

默认值为 `binary`。若不希望第一版扩大 CLI，可先只支持默认 binary，并在 SDK API 中保留 encoding options；但文档和错误信息必须明确实际支持范围。

### 11.3 文档

所有把 `.qpcd` 描述为生产格式的地方需要统一更新，至少包括：

- `README.md`
- `AGENTS.md` 中 storage 方向
- `docs/index.md`
- `docs/architecture.md`
- `docs/capture-store.md`
- `docs/implementation.md`
- `docs/zh/index.md`
- `docs/zh/architecture.md`
- `docs/zh/capture-store.md`
- `docs/zh/implementation.md`

## 12. 分阶段实施顺序

### 阶段 0：计划确认

本次提交即此阶段：

- 只新增本文档；
- 不修改代码；
- 不修改数据库；
- 不声称 PCD 已经可用。

### 阶段 1：依赖与互操作 spike

- 在隔离分支验证 `pcd-rs`、Rust 1.82、Windows MSVC。
- 生成包含 NaN、最大 `u16 ring`、organized dimensions 的小型 PCD fixture。
- 用独立工具读取 fixture。
- 记录 binary 与 binary-compressed 写入耗时和大小。
- 根据 gate 决定使用 `pcd-rs` 还是内部最小 codec。

输出必须是可复现测试或 benchmark 记录，不能只凭“crate 能编译”下结论。

### 阶段 2：新增 PCD codec，不切换生产路径

- 新增 `storage::pcd`。
- 完成 reader/writer 和单元测试。
- 保持 `capture-store` 仍写 `.qpcd`，便于并行对比同一帧。
- 不修改数据库保存抽象。

### 阶段 3：切换 `capture-store` 默认输出

- 默认扩展名改为 `.pcd`。
- 接入 viewpoint。
- 保留 temp + rename 和有界 storage queue。
- 更新现有 SQLite row 的 `cloud_path`、point count 等现有字段。
- 新数据不再写 `.qpcd`。

如果 richer metadata schema 尚未实施，不能删除 legacy header 信息而不记录缺口；应把必要 DB migration 与本阶段绑定，或推迟 production switch。

### 阶段 4：声明数据库保存接口，但不实现

- 新增 `repository.rs`。
- 声明 `ScanFrameMetadataStore::save_scan_frame_metadata`。
- 从 `storage::mod` 导出 trait/newtype。
- 不写任何 `impl`。
- 不改 `FramePersister` 调用点。
- 编译和文档测试通过。

### 阶段 5：legacy 和文档收尾

- 停用公开 `write_qpcd`。
- 保留并标注 `read_qpcd` legacy 用途。
- 更新中英文文档、输出目录示例和术语。
- 视已有数据量决定是否实现转换命令。

### 阶段 6：数据库后端实现，明确为后续任务

该阶段不属于当前请求：

- schema migration；
- `impl ScanFrameMetadataStore for SqliteStore`；
- 让 `FramePersister` 泛型化或使用 trait object；
- PostgreSQL 等新后端。

## 13. 测试与验证矩阵

### 13.1 PCD 单元测试

1. binary round-trip 保留所有 XYZIR 值。
2. `ring = 0` 和 `ring = u16::MAX` 正确。
3. NaN 坐标按 PCD 规则保留。
4. `WIDTH * HEIGHT == POINTS`。
5. organized cloud round-trip 保留维度。
6. 不一致维度被拒绝。
7. header 字段名称、顺序、SIZE/TYPE/COUNT 精确匹配契约。
8. VIEWPOINT translation/quaternion 顺序为 `tx ty tz qw qx qy qz`。
9. 非刚体/非有限 transform 被拒绝。
10. final path 已存在时不覆盖。
11. 写入失败后不留下 final 文件；temp 清理为 best effort。
12. reader 拒绝缺字段、重复字段、错误类型或 point count 截断。
13. 如果支持三种 encoding，分别 round-trip。

### 13.2 独立互操作验证

至少选择两个独立 reader：

- PCL reader/viewer；
- CloudCompare；
- PDAL；
- 另一种语言的 PCD parser。

验证：

- 文件能打开；
- point count 正确；
- intensity 和 ring 被识别为独立 scalar field；
- 坐标和单位正确；
- organized dimensions 未被破坏；
- binary-compressed 若启用，能由独立实现解压。

不能只用同一个 Rust crate 同时写和读作为“标准兼容”证据。

### 13.3 数据库/文件关联测试

在后续数据库实现阶段验证：

- DB `cloud_path` 指向真实 `.pcd`；
- DB point count、width、height 与 PCD header 一致；
- sequence/timestamp/source frame/coord frame 未丢失；
- duplicate `(session_id, sequence)` 不覆盖文件和 row；
- DB insert 失败时执行文件补偿清理；
- legacy `.qpcd` row 仍能通过格式分派读取。

### 13.4 实时性能验证

对代表性 Quanergy 帧测试：

- 平均和 P95/P99 写入延迟；
- 文件大小；
- CPU 使用率；
- storage queue occupancy；
- dropped frame 数；
- binary 与 binary-compressed 对比。

默认 encoding 的选择以“现场采集不增加不可接受丢帧”为优先，不以压缩率单项决定。

### 13.5 仓库级检查

```powershell
rtk cargo fmt --all -- --check
rtk cargo clippy --all-targets --all-features -- -D warnings
rtk cargo test --all-targets --all-features
```

## 14. 风险与缓解

| 风险 | 后果 | 缓解 |
| --- | --- | --- |
| 把 `.qpcd` 直接改扩展名 | 文件仍不是标准 PCD | 必须重写 header/data codec，并做独立互操作验证 |
| PCD 不含业务帧元数据 | timestamp/frame id/coord frame 丢失 | 数据库作为权威来源；schema 先迁移再切生产 |
| `VIEWPOINT` 方向或 quaternion 顺序错误 | 外部工具显示错误 sensor pose | 明确 `T_station_sensor` 和 `wxyz`，使用已知姿态 fixture |
| `pcd-rs` 与 MSRV/Windows 不兼容 | 构建失败 | dependency spike；保留内部固定 schema codec fallback |
| binary-compressed 增加 CPU | 有界队列溢出、丢帧 | 默认 binary；基准通过后才改变默认 |
| 旧 `.qpcd` reader 被删除 | 历史数据不可读 | 保留 legacy reader 和格式标识 |
| DB schema 无版本 | 已有数据库无法升级 | 先引入 `PRAGMA user_version` migration |
| 文件成功、DB 失败 | orphan 文件 | 补偿删除、可检测命名、后续 reconciliation |
| 覆盖同名 frame 文件 | 原有效数据被破坏 | final path 已存在即报错，不做 Windows 删除覆盖 fallback |
| 核心点类型耦合 codec | storage 依赖扩散 | 使用 `storage::pcd` 私有 adapter |

## 15. 验收标准

未来实施完成时，必须同时满足：

- [ ] `capture-store` 新生成的完整帧文件扩展名为 `.pcd`。
- [ ] 文件是 PCD 0.7，不是改名后的 QPCD。
- [ ] `FIELDS x y z intensity ring` 的类型和顺序符合本文契约。
- [ ] 独立工具能够读取 binary PCD。
- [ ] point count、organized dimensions、NaN、intensity、ring 无损。
- [ ] station-frame PCD 的 VIEWPOINT 与 `T_station_sensor` 一致。
- [ ] sequence、timestamp、source frame、coord frame、transform、calibration、qraw source 均可从数据库恢复。
- [ ] 实时有界队列和 dropped-frame 可观察性保持不变。
- [ ] 新数据不再调用 `write_qpcd`。
- [ ] 旧 `.qpcd` 至少仍可只读。
- [ ] `ScanFrameMetadataStore::save_scan_frame_metadata` 已声明并文档化。
- [ ] 该 trait 没有任何实现、默认 panic 或运行时调用，直到单独数据库任务开始。
- [ ] 不存在逐点数据库表或逐点 insert。
- [ ] Windows MSVC 构建、fmt、clippy、tests 全部通过。
- [ ] README、AGENTS 和中英文文档不再把 `.qpcd` 描述为默认生产格式。

## 16. 参考资料

- PCL 官方 PCD 格式说明：<https://pointclouds.org/documentation/tutorials/pcd_file_format.html>
- `pcd-rs` 文档：<https://docs.rs/pcd-rs/latest/pcd_rs/>

以上外部资料用于确认标准格式和候选实现能力；最终兼容性仍必须由仓库内 fixture、Windows 构建和独立 reader 验证。