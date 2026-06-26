# PR5: station-frame visualizer

## Source

`ref/plans/quanergy_tamping_station_y_axis_modification_plan.md` §12

## Goal

Rerun visualizer 支持站点坐标系显示：站点点云、XYZ 轴、Y 轴线、扫描仪原点/局部轴、hammer 中心/ROI。

## Requirements

- CLI 新增 `--station-config <FILE>`, `--show-sensor-frame`, `--show-hammer-rois`
- 未提供 station config：保持当前 sensor-frame 显示（向后兼容）
- 提供 station config：默认显示 station-frame 点云
- `--show-sensor-frame`：同时显示转换前点云
- Rerun 实体路径见计划 §12.2
- 新增 `VisualizerSink` trait：`log_static_station()` + `log_frame()`
- 静态场景只 log 一次
- 显示内容：O 点、XYZ 箭头、Y 轴延长线、扫描仪原点/局部轴、hammer 中心/AABB ROI/id 标签

## Acceptance Criteria

- [ ] 地面/平台处于合理 Z
- [ ] 捣固锤从小 Y 到大 Y 排序与现场一致
- [ ] 扫描仪位于预期高度
- [ ] 点云无镜像
- [ ] X/Y 方向无交换
- [ ] hammer ROI 覆盖目标
- [ ] 可选 sensor-frame 点云同时显示

## Files

```
apps/visualizer/src/lib.rs
apps/visualizer/src/rerun_sink.rs
docs/zh/visualizer.md
```
