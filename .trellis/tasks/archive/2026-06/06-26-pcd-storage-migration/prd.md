# PCD 点云存储迁移与数据库保存接口 — PRD

## Source

`docs/zh/pcd-storage-database-plan.md`（已批准的设计计划）

## Goal

1. 将 `capture-store` 的默认完整帧点云文件从私有 `.qpcd` 切换为标准 **PCD 0.7**。
2. 新增一个**只声明、不实现、不接线**的数据库保存接口 `ScanFrameMetadataStore::save_scan_frame_metadata`。
3. 保留旧 `.qpcd` 只读兼容路径。

## Non-goals

- 不把每个点写成数据库一行
- 不实现 PostgreSQL、PostGIS、pgPointcloud 或对象存储
- 不实现 ROI 分割、捣固锤高度计算或测量结果表
- 不改变 `.qraw` 录制、回放和 `.qraw.toml` sidecar 语义
- 不在 PCD comment 中创建另一套私有元数据协议

## Requirements

### R1: PCD 0.7 文件输出

- 生成符合 PCL PCD 0.7 header 和数据布局的 `.pcd` 文件
- 点字段固定为 `x y z intensity ring`，TYPE 为 `F F F F U`，SIZE 为 `4 4 4 4 2`
- 不保留 `.qpcd` 中始终为 `0` 的 `flags` 字段
- `binary` 为生产默认编码；`binary_compressed` 为 opt-in（需基准测试后方可设为默认）；`ascii` 仅调试用
- 保留 organized cloud 的 `WIDTH`/`HEIGHT` 语义
- 对非空且维度不一致的 frame，返回 `StorageFormat` 错误，不静默改写

### R2: VIEWPOINT 语义

- `VIEWPOINT` 使用 `T_station_sensor` 的平移和旋转（quaternion `wxyz` 顺序）
- 变换矩阵必须有限、近似正交，旋转行列式接近 `+1`
- 不能验证为刚体变换时返回错误
- 通用 PCD writer 不自行猜测坐标变换，由调用方通过 `PcdWriteOptions.viewpoint` 显式传入

### R3: 帧元数据分层

- PCD 文件只负责点数组、组织维度和 acquisition viewpoint
- `sequence`、时间戳、坐标系名称、变换、标定和来源路径等帧级元数据由数据库负责
- 区分 `source_frame_id`（输入 pipeline 的 frame 名）和 `coord_frame`（落盘坐标实际坐标系）

### R4: 数据库保存接口（仅声明）

- 新增 `crates/quanergy-client/src/storage/repository.rs`
- 声明 `ScanFrameMetadataStore` trait，含 `save_scan_frame_metadata(&mut self, frame: &NewScanFrame) -> Result<ScanFrameId>`
- 声明 `ScanFrameId(i64)` newtype
- **不写任何 `impl`**
- **不使用 `todo!()`、`unimplemented!()` 或 panic 默认方法**
- **不修改 `FramePersister` 调用点**
- 不删除现有 `SqliteStore::insert_scan_frame`

### R5: 旧 `.qpcd` 兼容

- 停止生成新的 `.qpcd`
- 旧实现移动或重命名为 `storage::legacy_qpcd`
- 保留 `read_qpcd` 只读能力
- 公共 API 对 legacy reader 标记 deprecated

### R6: 实时存储约束

- 保持有界队列和显式 backpressure 行为
- 临时文件写入 + rename 原子提交
- 存量 final 文件不可覆盖
- 写入失败后不留下 final 文件

### R7: 第三方 crate gate

- 优先评估 `pcd-rs`；若任何关键 gate 失败，fallback 为内部最小 PCD 0.7 固定 XYZIR codec
- Gate: Rust 1.82 MSRV + Windows MSVC 构建 + u16 ring/NaN/organized/三编码 round-trip + 独立工具可读 + 依赖树和许可证可接受

### R8: 不污染核心点类型

- 不在 `cloud::PointXyzir` 上派生第三方 `PcdSerialize/PcdDeserialize`
- 在 `storage::pcd` 内部使用私有 adapter 做转换

## Acceptance Criteria

- [ ] `capture-store` 新生成的完整帧文件扩展名为 `.pcd`
- [ ] 文件是 PCD 0.7，不是改名后的 QPCD
- [ ] `FIELDS x y z intensity ring` 的类型和顺序符合契约
- [ ] 独立工具（PCL/CloudCompare/PDAL）能够读取 binary PCD
- [ ] point count、organized dimensions、NaN、intensity、ring 无损
- [ ] station-frame PCD 的 VIEWPOINT 与 `T_station_sensor` 一致
- [ ] sequence、timestamp、source frame、coord frame、transform、calibration、qraw source 均可从数据库恢复
- [ ] 实时有界队列和 dropped-frame 可观察性保持不变
- [ ] 新数据不再调用 `write_qpcd`
- [ ] 旧 `.qpcd` 至少仍可只读
- [ ] `ScanFrameMetadataStore::save_scan_frame_metadata` 已声明并文档化
- [ ] 该 trait 没有任何实现、默认 panic 或运行时调用
- [ ] 不存在逐点数据库表或逐点 insert
- [ ] Windows MSVC 构建、fmt、clippy、tests 全部通过
- [ ] README、AGENTS 和中英文文档不再把 `.qpcd` 描述为默认生产格式

## Files

```
crates/quanergy-client/src/storage/pcd.rs              (新增)
crates/quanergy-client/src/storage/qpcd.rs             (迁移为 legacy)
crates/quanergy-client/src/storage/repository.rs       (新增，仅声明)
crates/quanergy-client/src/storage/mod.rs              (导出 PCD API，弃用 legacy)
crates/quanergy-client/src/storage/metadata.rs         (增加 PCD 字段)
crates/quanergy-client/src/error.rs                    (codec 错误映射)
apps/capture-store/src/main.rs                         (write_qpcd → write_pcd)
docs/**                                                (更新术语)
```
