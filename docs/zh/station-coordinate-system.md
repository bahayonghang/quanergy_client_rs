# 站点坐标系规范

## 坐标系定义

捣固站使用右手直角坐标系，定义如下：

| 轴 | 方向 | 说明 |
|---|---|---|
| `+Z` | 竖直向上 | 高度方向 |
| `+Y` | 沿捣固锤排列方向 | 锤的固定排列轴 |
| `+X` | 水平面内与 Y 轴垂直 | 满足右手系 |
| 原点 `O` | 现场选定的站点基准点 | 空间坐标原点 |

## frame_id

| 名称 | frame_id | 含义 |
|---|---|---|
| 扫描仪坐标系 | `quanergy_sensor` | SensorPipeline 输出点所在坐标系 |
| 站点坐标系 | `station` | 固定站点 O-XYZ 坐标系 |
| 第 i 个捣固锤 | `hammer_<id>` | 名义中心位于 Y 轴附近，轴线平行 Z |

## 从扫描仪到站点的固定变换

对扫描仪坐标点 `p_s`，站点坐标为：

```
p_station = T_station_sensor * p_s
```

其中 `T` 为 4×4 刚体矩阵，满足：
- `RᵀR ≈ I`（正交）
- `det(R) ≈ +1`（右手系，无镜像）
- 末行 `[0, 0, 0, 1]`
- 所有元素有限

## 垂直向下安装的注意事项

"扫描仪垂直向下"不能通过只反转 Z 得到：

```
diag(1, 1, -1)   # 错误：这是镜像，det = -1
```

必须使用合法旋转矩阵。候选初值（需现场标定确认）：

```
T_station_sensor =
  [[1,  0,  0, 0.20],
   [0, -1,  0, 0.68],
   [0,  0, -1, 7.85],
   [0,  0,  0, 1.00]]
```

该矩阵等效于 `roll = 180°`、`yaw = 0°`、`pitch = 0°`：
- `sensor +X` → `station +X`
- `sensor +Y` → `station -Y`
- `sensor +Z` → `station -Z`

## 单位约定

- 长度：米（m）
- 角度：配置使用度（°），内部计算使用弧度
- 点云坐标：`f32`
- 配置解析/标定：`f64`

## station.toml 配置

参见 `config/station.example.toml`。配置必须指定：
- schema_version = 1
- frames.source / frames.target
- scanner.extrinsic（4×4 矩阵）
- hammers（Y 轴布局）
