# PR3: 存储可靠性和 schema v2

## Source

`ref/plans/quanergy_tamping_station_y_axis_modification_plan.md` §9–§10

## Goal

QPCD header 新增 provenance 字段，SQLite 迁移到 v2，存储 worker 实现 JoinHandle + error 回传 + graceful shutdown。

## Requirements

- QPCD header 新增可选字段：source_frame, target_frame, station_id, transform_id, station_config_sha256
- 保持 20 字节点步长和 QPCDv1 二进制兼容
- 新增 `write_qpcd_with_metadata` API，保留旧 `write_qpcd`
- SQLite 引入 `PRAGMA user_version` 迁移机制（v1→v2）
- capture_session 新增字段见计划 §10.2
- scan_frame 新增 source_frame / target_frame
- worker 消息协议：`StorageCommand::Persist` / `Finish`
- 主线程保存 `JoinHandle<Result<StorageStats>>`
- graceful shutdown 顺序见计划 §8.4

## Acceptance Criteria

- [ ] 旧 QPCD 仍可读取
- [ ] 新 QPCD header round trip 正确
- [ ] 新建数据库为最新 schema
- [ ] v1 数据库可迁移到 v2，重复迁移幂等
- [ ] Ctrl+C 后 worker 正常 join
- [ ] session 状态 correct：complete / aborted / failed
- [ ] 单元测试覆盖计划 §15.4–§15.5

## Files

```
crates/quanergy-client/src/storage/qpcd.rs
crates/quanergy-client/src/storage/sqlite.rs
crates/quanergy-client/src/storage/migrations.rs     (新增)
apps/capture-store/src/worker.rs                     (新增)
```
