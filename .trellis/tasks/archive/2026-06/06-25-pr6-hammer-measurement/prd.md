# PR6: 静态 Y 轴锤测量

## Source

`ref/plans/quanergy_tamping_station_y_axis_modification_plan.md` §13

## Goal

实现 Y 轴静态 ROI 分割、top Z 稳健估计、SQLite measurement 表、CSV 导出、tamping-analyzer 离线应用。

## Requirements

- 新增 `crates/quanergy-client/src/measure/` 模块（hammer_roi.rs, height.rs, result.rs）
- ROI 分割：按固定 Y 坐标、X 带宽、Z 范围裁剪
- 重叠 ROI 按最近 Y 中心分配，记录 overlap warning
- Top Z 估计器：顶部比例点中位数（第一版）
- 输出：top_z_m, roi_point_count, valid_point_count, z_spread_m, quality
- 配置参考面后输出 height = top_z - reference_z
- 新增 `apps/tamping-analyzer/` 离线应用（session 子命令 + CSV 导出）
- SQLite 新增 `hammer_measurement` 表（计划 §10.4）

## Acceptance Criteria

- [ ] 已知 Y 坐标目标正确分割
- [ ] 重叠 ROI 决策可重复
- [ ] 禁用锤不参与分割
- [ ] 点数不足返回 invalid
- [ ] CSV 输出可读
- [ ] fixture tests 覆盖

## Files

```
crates/quanergy-client/src/measure/          (新增)
crates/quanergy-client/src/lib.rs            (pub mod measure)
apps/tamping-analyzer/                       (新增)
crates/quanergy-client/src/storage/sqlite.rs  (hammer_measurement 表)
```
