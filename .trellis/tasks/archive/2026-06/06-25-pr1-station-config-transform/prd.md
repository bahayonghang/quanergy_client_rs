# PR1: 站点配置与刚体变换

## Source

`ref/plans/quanergy_tamping_station_y_axis_modification_plan.md` §6–§7

## Goal

新增 `station` 模块，实现 station.toml 配置加载/校验/哈希、刚体矩阵校验、StationTransform 带 frame 语义的点云转换。

## Requirements

- 新增 `crates/quanergy-client/src/station/` 模块（config.rs, geometry.rs, hammer.rs）
- TOML 配置模型：`StationConfig` → `ValidatedStationConfig`，校验规则见计划 §6.1
- 新增 `transform/validation.rs`：刚体矩阵校验（正交性、行列式、末行、有限值）
- 新增 `StationTransform`：带 source_frame/target_frame/transform_id 的转换包装
- `transform_frame_to_target()` 转换 XYZ、保留 intensity/ring、修正 frame_id
- 候选外参初值见计划 §4.4
- 数值精度：配置解析用 f64，点云路径用 f32

## Acceptance Criteria

- [ ] `config/station.example.toml` 可解析并通过校验
- [ ] 镜像矩阵 `det < 0` 被拒绝
- [ ] 非正交矩阵被拒绝
- [ ] 未知字段报错
- [ ] 重复 hammer id 报错
- [ ] NaN/Inf 坐标报错
- [ ] 变换后 `frame_id == target_frame`
- [ ] intensity/ring/sequence/timestamp 不变
- [ ] 单元测试覆盖计划 §15.1–§15.3
- [ ] `rtk cargo test --all-targets --all-features` 通过

## Files

```
crates/quanergy-client/src/lib.rs          (新增 pub mod station)
crates/quanergy-client/src/station/        (新增)
crates/quanergy-client/src/transform/      (新增 validation.rs)
config/station.example.toml                (新增)
```
