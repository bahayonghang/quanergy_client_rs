# 捣固站 Y 轴静态建模 — 总协调

## Goal

将 Quanergy 三维激光扫描仪固定安装于捣固站上方，实现：
1. 固定刚体变换将传感器点云转换到站点坐标系
2. 站点坐标点云本地持久化（QPCD + SQLite）
3. Rerun 中显示站点坐标点云及静态站点几何
4. 后续按静态 Y 轴位置分割捣固锤并计算 Z 向高度

## Source Plan

`ref/plans/quanergy_tamping_station_y_axis_modification_plan.md` — 本任务的所有需求、设计、实施细节均以此计划为准。

## Key Constraints

- **无外轴**：捣固锤沿站点 Y 轴静态排列，不存在 `q(t)`、外轴编码器、运动学或动态外参
- **固定变换**：`T_station_sensor` 在整个 session 内不变
- **右手坐标系**：`det(R) = +1`，禁止镜像变换
- **Windows 优先**：`x86_64-pc-windows-msvc`，不因 Linux/macOS 兼容性阻碍第一版
- **不破坏现有 API**：保留旧 QPCD、旧 CLI 参数（带 deprecation warning）
- **正式模式 zero tolerance**：不允许静默丢帧、标定回退、或伪造测量结果

## Task Map

| 序号 | 子任务 | 依赖 | 说明 |
|------|--------|------|------|
| PR1 | `pr1-station-config-transform` | — | station.toml + 刚体矩阵校验 + StationTransform |
| PR2 | `pr2-capture-store-station` | PR1 | CLI --station-config + 固定变换 + session 快照 |
| PR3 | `pr3-storage-reliability` | PR2 | QPCD provenance / SQLite v2 migration / worker JoinHandle |
| PR4 | `pr4-raw-recorder` | PR3 | raw recorder worker / queue fail-fast / require-device-info |
| PR5 | `pr5-station-visualizer` | PR1 | Rerun 站点坐标 / 轴 / 扫描仪 / hammer ROI |
| PR6 | `pr6-hammer-measurement` | PR1–PR5 | ROI 分割 / top Z estimator / tamping-analyzer |
| PR7 | `pr7-field-calibration` | PR1–PR6 | 外参标定 / 几何复核 / 运维文档 |

## Cross-Child Acceptance Criteria (MVP)

- [ ] 代码中不存在外轴 `q(t)`、外轴编码器或动态外参模型
- [ ] hammer 模型明确为沿站点 Y 轴静态排列
- [ ] `station.toml` 可加载、校验和哈希
- [ ] 外参矩阵通过刚体校验
- [ ] live 和 replay 都使用同一固定外参
- [ ] 变换后点云 `frame_id == "station"`
- [ ] `.qpcd` 保存站点坐标点
- [ ] SQLite 保存 station config、外参和标定快照
- [ ] 可选 `.qraw` 完整保存并可回放
- [ ] 正式模式不静默丢帧
- [ ] Ctrl+C 后文件 flush、worker join、session 状态正确
- [ ] visualizer 显示站点 XYZ、Y 轴、扫描仪和 hammer ROI
- [ ] 已知几何验证无镜像、无 X/Y 交换
- [ ] Windows release 构建、fmt、clippy 和 test 全部通过
- [ ] 文档和示例配置同步更新

## CI Quality Gate (per child PR)

```powershell
rtk cargo fmt --all -- --check
rtk cargo clippy --all-targets --all-features -- -D warnings
rtk cargo test --all-targets --all-features
rtk cargo build --release -p capture-store
rtk cargo build --release -p visualizer
```

## Notes

- 实施顺序必须固定：station.toml → 刚体矩阵校验 → capture-store 固定变换 → frame/provenance 修正 → 存储可靠性 → station visualizer → Y 轴 hammer ROI → 高度/行程测量
- 不得先开发高度算法再补坐标系和外参
- 每个子任务独立可验证，依赖关系写入子任务 PRD
