# PR2: capture-store 接入 station config

## Source

`ref/plans/quanergy_tamping_station_y_axis_modification_plan.md` §8

## Goal

capture-store 支持 `--station-config`，使用固定 StationTransform 替代旧六参数变换，输出站点坐标点云。

## Requirements

- 新增 `--station-config <FILE>` 全局参数
- 新增 `validate-station-config <FILE>` 子命令
- `--station-config` 与旧六参数（--x-m 等）互斥，旧参数输出 deprecation warning
- live/replay 均使用 `StationTransform::transform_frame_to_target()`
- 存储时 `coord_frame == frame_id == "station"`
- session config snapshot（拷贝 station.toml + SHA-256）
- 建议拆分 `apps/capture-store/src/` 为 cli.rs / live.rs / replay.rs / session.rs / worker.rs

## Acceptance Criteria

- [ ] `--station-config config/station.toml` 可正常启动 live 采集
- [ ] `validate-station-config` 正确报告配置错误
- [ ] 旧六参数仍可用但输出 warning
- [ ] 新旧参数互斥生效
- [ ] 输出 QPCD 中 coord_frame 为 "station"
- [ ] session 目录包含 station.toml 副本
- [ ] `rtk cargo build --release -p capture-store` 通过

## Files

```
apps/capture-store/src/    (重构 + 新增 CLI)
```
