# Tamping Analyzer

离线捣固锤高度测量工具。

## 命令

```powershell
rtk cargo run --release -p tamping-analyzer -- \
  --database <DATABASE> \
  --session-id <SESSION_ID> \
  --station-config <STATION_TOML> \
  --output-csv <OUTPUT_CSV>
```

## 参数

| 参数 | 说明 |
|---|---|
| `--database`, `-d` | capture session SQLite 数据库路径 |
| `--session-id`, `-s` | 要分析的 session ID |
| `--station-config` | station.toml 路径（含 hammer 布局） |
| `--output-csv`, `-o` | 输出 CSV 文件（默认 stdout） |
| `--top-ratio` | 高度估计器顶部比例（默认 0.1 = top 10%） |
| `--min-points` | 单锤有效最小点数（默认 10） |

## 流程

1. 从 SQLite 读取 session 的所有帧（`list_scan_frames`）
2. 加载 station.toml 获取 hammer 布局（`HammerLayout`）
3. 对每帧：
   - 读取 QPCD 点云
   - `segment_frame()` 按最近 Y 中心分割为各锤 Z 值
   - `measure_hammer()` 计算 top_z_m、quality 等
   - 结果写入 SQLite `hammer_measurement` 表
4. 计算 per-hammer 跨帧统计
5. 输出 CSV

## 输出 CSV 格式

```csv
hammer_id,frame_count,valid_frame_count,mean_top_z_m,std_top_z_m,min_top_z_m,max_top_z_m,mean_point_count
H01,150,148,2.345,0.012,2.320,2.370,523.4
H02,150,150,2.351,0.008,2.335,2.368,511.2
```

## 高度估计器

默认使用 top-percentile median：
1. 过滤 NaN Z 值
2. 按 Z 降序排列
3. 取顶部 `top_ratio` 比例的点
4. 返回这些点的中位数

配置 `reference_z_m` 后（在 station.toml 中通过 `z_min_m` 等定义），height = top_z - reference_z。

## SQLite 表

测量结果写入 `hammer_measurement` 表（schema v3）：

```sql
CREATE TABLE hammer_measurement (
    measurement_id    INTEGER PRIMARY KEY,
    session_id        TEXT NOT NULL,
    sequence          INTEGER NOT NULL,
    hammer_id         TEXT NOT NULL,
    roi_point_count   INTEGER NOT NULL,
    valid_point_count INTEGER NOT NULL DEFAULT 0,
    top_z_m           REAL,
    reference_z_m     REAL,
    height_m          REAL,
    z_spread_m        REAL,
    quality           REAL NOT NULL DEFAULT 0.0,
    estimator         TEXT NOT NULL,
    status            TEXT NOT NULL,
    created_at        TEXT NOT NULL,
    UNIQUE(session_id, sequence, hammer_id)
);
```
