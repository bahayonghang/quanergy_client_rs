# 外参标定

## 工具

使用 `station-calibrate` 工具进行刚体变换外参标定：

```powershell
rtk cargo run --release -p station-calibrate -- \
  --calibration-id station-01-scanner-2026-06-25 \
  --method field_arun \
  --calibrated-by "姓名" \
  -o station.toml \
  targets.csv
```

## 输入格式

CSV 文件，每行一个对应点：

```csv
# sensor_x, sensor_y, sensor_z, station_x, station_y, station_z
1.0, 0.0, 0.0, 1.20, 0.68, 7.85
0.0, 1.0, 0.0, 0.20, -0.32, 7.85
0.0, 0.0, 1.0, 0.20, 0.68, 6.85
```

以 `#` 开头的行为注释。

## 标定要求

### 最低要求

- 至少 3 个不共线对应点
- 建议 6 个以上，覆盖：
  - 不同 X
  - 不同 Y
  - 不同 Z
  - 扫描区域主要范围

### 几何复核

完成标定后必须验证：

1. 已知水平面转换后 Z 近似常数
2. 沿现场 +Y 放置的标靶转换后 Y 递增
3. 沿现场 +X 放置的标靶转换后 X 递增
4. 扫描仪原点映射到实测安装位置
5. `det(R)` 为正（右手系）
6. Rerun 中无镜像、无 X/Y 交换

### 验收阈值（建议）

| 指标 | 阈值 |
|---|---|
| 外参标定 RMS | ≤ 0.02 m |
| 外参最大 residual | ≤ 0.05 m |
| 已知 Y 向标靶顺序 | 100% 正确 |

## 标定流程

1. 在现场放置至少 6 个反射标靶，覆盖扫描区域
2. 使用全站仪或已知安装尺寸测量每个标靶的站点坐标
3. 使用 `visualizer` 的 `--show-sensor-frame` 模式记录每个标靶在传感器坐标系中的位置
4. 整理为 targets.csv
5. 运行 `station-calibrate` 求解
6. 检查 RMS/max error 是否在阈值内
7. 检查 det(R) > 0（右手系）
8. 将输出的 TOML 保存为 `config/station.toml`
9. 使用 `validate-station-config` 校验
10. 在 Rerun 中验证几何

## 算法

使用 Arun 的 SVD 方法（Kabsch 算法）：

1. 计算两组点的质心
2. 中心化
3. 计算交叉协方差矩阵 H = Σ s_i * t_i^T
4. SVD 分解：H = U Σ V^T
5. 旋转矩阵：R = V U^T
6. 若 det(R) < 0，修正（反转 V 最后一列）
7. 平移：t = centroid_target - R * centroid_source
