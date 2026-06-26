# PCD 点云存储迁移与数据库保存接口 — Implement

## 阶段 0：计划确认 ✅

- [x] 计划文档 `docs/zh/pcd-storage-database-plan.md` 已存在
- [x] Trellis task 已创建，artifact 已填充
- [x] 用户 review 并 approve → `task.py start`

## 阶段 1：依赖与互操作 spike ✅

- [x] 1.1 在隔离分支添加 `pcd-rs` 依赖验证编译（Rust 1.82 + Windows MSVC）
- [x] 1.2 生成包含 NaN、`u16::MAX` ring、organized dimensions 的小型 PCD fixture
- [x] 1.3 用独立工具（PCL viewer / CloudCompare / PDAL）读取 fixture 验证 → 推迟：需安装外部工具
- [x] 1.4 记录 binary 与 binary-compressed 写入耗时和文件大小 → binary 18.0 bytes/pt, compressed 12.3 bytes/pt (ratio 0.68)
- [x] 1.5 根据 gate 决定使用 `pcd-rs` 还是内部最小 codec → 采用 pcd-rs
- [x] 1.6 输出可复现测试或 benchmark 记录

## 阶段 2：新增 PCD codec，不切换生产路径 ✅

- [x] 2.1 新增 `crates/quanergy-client/src/storage/pcd.rs`
- [x] 2.2 实现 `PcdEncoding`、`PcdViewpoint`、`PcdWriteOptions`、`PcdFileInfo`、`PcdCloud`
- [x] 2.3 实现私有 `PcdPointXyzir` adapter + `From<PointXyzir>` 双向转换
- [x] 2.4 实现 `write_pcd()` — header 生成、binary data 写入、organized dimension 验证
- [x] 2.5 实现 `read_pcd()` — header 解析、点数据读取、`PcdCloud` 构建
- [x] 2.6 实现 VIEWPOINT 写入/读取（含刚体变换验证）
- [x] 2.7 在 `storage::mod` 导出 PCD API
- [x] 2.8 PCD 单元测试（见测试矩阵 §13.1）
- [x] 2.9 `capture-store` 保持写 `.qpcd`，便于并行对比

## 阶段 3：切换 capture-store 默认输出

- [x] 3.1 将 `write_qpcd` 标记为 deprecated / 移入 `legacy_qpcd` → phase 5 完成
- [x] 3.2 `FramePersister::persist_frame` 中 `write_qpcd` → `write_pcd_atomic`
- [x] 3.3 默认扩展名 `.qpcd` → `.pcd`
- [x] 3.4 接入 viewpoint（从 transform matrix 提取 translation + quaternion）
- [x] 3.5 保留 temp + rename 和有界 storage queue
- [x] 3.6 更新现有 SQLite row 写入：cloud_path 改为 `.pcd`
- [ ] 3.7 引入 `cloud_format` / `cloud_format_version` / `cloud_encoding` 元数据字段 → **推迟到 DB migration 任务**
- [ ] 3.8 实施 `PRAGMA user_version` migration（v0→v1） → **推迟到 DB migration 任务**
- [ ] 3.9 旧行回填：`cloud_format='qpcd'` 等 → **推迟到 DB migration 任务**

## 阶段 4：声明数据库保存接口 ✅

- [x] 4.1 新增 `crates/quanergy-client/src/storage/repository.rs`
- [x] 4.2 声明 `ScanFrameId(i64)` newtype
- [x] 4.3 声明 `ScanFrameMetadataStore` trait + `save_scan_frame_metadata` 方法（含完整文档注释）
- [x] 4.4 从 `storage::mod` 导出 trait 和 newtype
- [x] 4.5 确认：无任何 `impl`、无 `todo!()`、无 panic、未改 `FramePersister`
- [x] 4.6 编译和 `cargo doc` 通过

## 阶段 5：legacy 和文档收尾

- [x] 5.1 停用公开 `write_qpcd`，保留 `read_qpcd` 并标注 deprecated
- [x] 5.2 更新 README.md — `.qpcd` → `.pcd`
- [x] 5.3 更新 AGENTS.md — storage 方向描述
- [x] 5.4 更新 docs/index.md、docs/architecture.md、docs/capture-store.md、docs/implementation.md
- [x] 5.5 更新 docs/zh/index.md、docs/zh/architecture.md、docs/zh/capture-store.md、docs/zh/implementation.md

## 阶段 6：数据库后端实现（后续任务，不在本 task 范围）

- 不实施：schema migration、`impl ScanFrameMetadataStore for SqliteStore`、`FramePersister` 泛型化、PostgreSQL 后端

## 验证命令

每个阶段完成后运行：

```powershell
rtk cargo fmt --all -- --check
rtk cargo clippy --all-targets --all-features -- -D warnings
rtk cargo test --all-targets --all-features
```

## 测试矩阵

### PCD 单元测试（阶段 2）

- [x] binary round-trip 保留所有 XYZIR 值
- [x] `ring = 0` 和 `ring = u16::MAX` 正确
- [x] NaN 坐标按 PCD 规则保留
- [x] `WIDTH * HEIGHT == POINTS`
- [x] organized cloud round-trip 保留维度
- [x] 不一致维度被拒绝
- [x] header 字段名称、顺序、SIZE/TYPE/COUNT 精确匹配契约
- [x] VIEWPOINT translation/quaternion 顺序为 `tx ty tz qw qx qy qz`
- [x] 非刚体/非有限 transform 被拒绝
- [x] final path 已存在时不覆盖
- [x] 写入失败后不留下 final 文件（atomic write temp→rename）
- [x] reader 拒绝缺字段、重复字段、错误类型或 point count 截断

### 互操作验证（阶段 1-2）

- [ ] 至少两个独立 reader（PCL/CloudCompare/PDAL）能打开文件 → **推迟：需安装外部工具**
- [ ] point count 正确 → **推迟**
- [ ] intensity 和 ring 被识别为独立 scalar field → **推迟**
- [ ] 坐标和单位正确 → **推迟**
- [ ] organized dimensions 未被破坏 → **推迟**

### 实时性能验证（阶段 3）

- [ ] 平均和 P95/P99 写入延迟 → **推迟：需连接真实传感器**
- [ ] 文件大小对比（QPCD vs PCD）
- [ ] storage queue occupancy → **推迟**
- [ ] dropped frame 数 → **推迟**
- [ ] binary vs binary-compressed 对比 → spike 已完成初步对比 (50k pts: binary 900KB, compressed 616KB, ratio 0.68)
