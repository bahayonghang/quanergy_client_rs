# PR4: 原始录制可靠性

## Source

`ref/plans/quanergy_tamping_station_y_axis_modification_plan.md` §8.5–§8.7

## Goal

将 .qraw 写入从主循环拆出为独立 worker，实现队列溢出策略、require-device-info 严格模式、raw completeness 追踪。

## Requirements

- raw recorder worker：`ingestion → bounded raw queue → raw recorder worker`
- 队列溢出策略：fail（正式默认）/ drop-newest（监看）/ block（离线回放）
- `--require-device-info`：deviceInfo 失败时拒绝开始正式采集
- `--overflow-policy <fail|drop-newest|block>`
- `--storage-queue-capacity <N>` 可配置
- worker 错误回传主线程
- session 结束时 flush
- sidecar 记录 deviceInfo/标定完整性
- raw 录制失败时 session 不得标记 complete

## Acceptance Criteria

- [ ] 正式模式 raw queue 满时 session → failed
- [ ] drop-newest 模式记录 dropped frame 数
- [ ] --require-device-info 生效（无 deviceInfo → 拒绝启动）
- [ ] sidecar 标定不完整时写入显式标记
- [ ] raw worker panic 被捕获并回传

## Files

```
apps/capture-store/src/worker.rs
apps/capture-store/src/session.rs
```
