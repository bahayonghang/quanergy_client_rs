# PCD 点云存储迁移与数据库保存接口 — Design

## 1. PCD 文件契约

参考 PCL PCD 0.7：header 为 ASCII，字段顺序固定，数据可为 `ascii`、`binary` 或 `binary_compressed`。

### 1.1 固定点 schema

| PCD field | SIZE | TYPE | COUNT | Rust 类型 |
|-----------|------|------|-------|-----------|
| `x`       | 4    | F    | 1     | `f32`     |
| `y`       | 4    | F    | 1     | `f32`     |
| `z`       | 4    | F    | 1     | `f32`     |
| `intensity`| 4    | F    | 1     | `f32`     |
| `ring`    | 2    | U    | 1     | `u16`     |

`binary` 下每点有效 payload 18 字节。不人为加入无意义 padding field。

空点云：`WIDTH 0` / `HEIGHT 1` / `POINTS 0`。

### 1.2 VIEWPOINT

格式：`VIEWPOINT tx ty tz qw qx qy qz`

- translation 使用 `T_station_sensor` 的平移部分
- quaternion 使用左上 3x3 旋转矩阵 → `qw qx qy qz`
- 变换矩阵必须有限、近似正交，行列式接近 `+1`

## 2. 公共 API 设计

```rust
// crates/quanergy-client/src/storage/pcd.rs

pub enum PcdEncoding { Ascii, Binary, BinaryCompressed }

pub struct PcdViewpoint { pub translation_m: [f32; 3], pub rotation_wxyz: [f32; 4] }

pub struct PcdWriteOptions { pub encoding: PcdEncoding, pub viewpoint: PcdViewpoint }

pub struct PcdFileInfo { pub point_count: u64, pub width: usize, pub height: usize, pub encoding: PcdEncoding, pub file_size_bytes: u64 }

pub struct PcdCloud { pub width: usize, pub height: usize, pub viewpoint: PcdViewpoint, pub points: Vec<PointXyzir> }

pub fn write_pcd(path: impl AsRef<Path>, frame: &Frame<PointXyzir>, options: &PcdWriteOptions) -> Result<PcdFileInfo>;
pub fn read_pcd(path: impl AsRef<Path>) -> Result<PcdCloud>;
```

### 2.1 设计决策

- `read_pcd` 不直接返回 `Frame<PointXyzir>`——PCD 文件不含 `stamp_micros`、`sequence`、`source_frame_id`。正确路径：`ScanFrameRecord (DB) + PcdCloud (file) → validated Frame`
- 不在 `cloud::PointXyzir` 上派生第三方 trait，使用私有 adapter 隔离

```rust
// 私有 adapter
struct PcdPointXyzir { x: f32, y: f32, z: f32, intensity: f32, ring: u16 }
```

## 3. 第三方 crate 选择

优先评估纯 Rust `pcd-rs`。Gate 列表：

1. Rust 1.82 MSRV 下编译通过
2. `x86_64-pc-windows-msvc` 构建和测试通过
3. `u16 ring`、NaN、organized dimensions 和三个 encoding round-trip 正确
4. 产物可由独立 PCL/CloudCompare/PDAL 工具读取
5. dependency tree 和许可证可接受
6. writer 的 `finish()`、seek、错误行为满足临时文件写入流程

Fallback：内部最小 PCD 0.7 固定 XYZIR codec。第一版可只实现 `binary`，压缩另开任务。

## 4. Database 保存接口（仅声明）

```rust
// crates/quanergy-client/src/storage/repository.rs

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
    fn save_scan_frame_metadata(&mut self, frame: &NewScanFrame) -> Result<ScanFrameId>;
}
```

### 4.1 设计决策

- `&mut self`：明确单 writer、顺序提交和未来 transaction 状态
- `Send`：允许 store 被移动到 storage worker；不要求 `Sync`
- 同步 trait：`capture-store` 已在独立 storage worker thread 中执行阻塞 I/O
- `ScanFrameId` newtype：避免上层直接绑定 SQLite 的 `last_insert_rowid`

### 4.2 明确禁止

- 不写 `impl ScanFrameMetadataStore for SqliteStore`
- 不提供返回固定值的假实现
- 不使用 `todo!()`、`unimplemented!()` 或 panic 默认方法
- 不修改 `FramePersister` 调用点

## 5. 帧元数据归属

| 信息 | PCD | 数据库 |
|------|:---:|:---:|
| format_version | `VERSION 0.7` | cloud_format/version |
| point_count | `POINTS` | point_count |
| width/height | `WIDTH`/`HEIGHT` | 建议冗余保存 |
| stamp_micros | 否 | 是 |
| sequence | 文件名提示 | 是（权威） |
| coord_frame | 否 | 是（权威） |
| frame_id | 否 | source_frame_id |
| transform | VIEWPOINT (仅 pose) | 完整 4x4 + JSON |
| calibration | 否 | 是（权威） |
| qraw source | 否 | 是（权威） |

## 6. 持久化流程

```text
1. 验证 Frame dimensions、点数和 transform/viewpoint
2. 构造 final path: frame_<sequence>.pcd
3. 检查 final path 不存在（重复 sequence 报错）
4. 写 frame_<sequence>.pcd.tmp
5. writer.finish()
6. flush / 必要时 sync
7. 重新读取最小 header，验证 schema、POINTS、WIDTH、HEIGHT
8. rename tmp → final
9. 构造 NewScanFrame，cloud_path 指向 final .pcd
10. 调用现有 SQLite insert；未来切换到 save_scan_frame_metadata trait
11. 成功后记录 frame committed
```

## 7. 错误处理矩阵

| 失败点 | 数据库 | 文件处理 |
|--------|--------|----------|
| validation 失败 | 无记录 | 不创建文件 |
| temp 写入失败 | 无记录 | 删除 temp |
| finish/flush 失败 | 无记录 | 删除 temp |
| header 自检失败 | 无记录 | 删除 temp |
| rename 失败 | 无记录 | 保留或删除 temp |
| DB insert 失败 | 无记录 | 删除 final（best effort） |
