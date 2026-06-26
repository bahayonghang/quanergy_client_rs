# PR7: 现场标定与文档

## Source

`ref/plans/quanergy_tamping_station_y_axis_modification_plan.md` §14, §18

## Goal

外参标定工具、几何复核流程、生产配置生成、运维文档。

## Requirements

- 外参对应点采集与求解（≥6 点，覆盖不同 X/Y/Z）
- 输出：4×4 外参、RMS/max error、各标靶 residual、calibration id
- 几何复核：水平面 Z 常数、Y 递增、X 递增、无镜像
- 候选标定工具（apps/station-calibrate/ 或 tamping-analyzer calibrate-extrinsic）
- 标定工具只输出候选 TOML，不直接覆盖生产配置
- 新增文档见计划 §18

## Acceptance Criteria

- [ ] 外参 RMS ≤ 0.02 m
- [ ] 外参最大 residual ≤ 0.05 m
- [ ] 已知 Y 向标靶顺序 100% 正确
- [ ] 生产 `station.toml` 就绪
- [ ] 运维说明文档完成

## Files

```
apps/station-calibrate/    (新增，或扩展 tamping-analyzer)
docs/zh/station-coordinate-system.md
docs/zh/station-calibration.md
docs/zh/tamping-analyzer.md
config/station.toml        (生产配置)
```
