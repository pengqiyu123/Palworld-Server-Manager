# 幻兽帕鲁服务器配置调研：休闲、正常、困难三种体验

## 概述

本调研基于 **1.0 正式版**（2026-07-09 发布），参考社区玩家实测经验、B站高热度攻略和官方文档，整理出三种难度体验的完整参数配置。

> **三档定位**：休闲党（护肝爽玩）/ 正常党（原汁原味微调）/ 困难党（挑战自我）

## 配置文件位置

| 平台 | 文件路径 |
|------|----------|
| Windows | `Pal\Saved\Config\WindowsServer\PalWorldSettings.ini` |
| Linux | `Pal/Saved/Config/LinuxServer/PalWorldSettings.ini` |

---

## 三档快速对照总表

| 参数 | 休闲档 🌿 | 正常档 ⚖️ | 困难档 🔥 | 参数说明 |
|------|----------|----------|----------|----------|
| **难度预设** | Casual | Normal | Hard | 影响多项底层数值 |
| **死亡惩罚** | None | ItemAndEquipment | All | 越高越硬核 |
| **经验值倍率** | 1.8 | 1.2 | 0.8 | 越高升级越快 |
| **帕鲁经验倍率** | 1.5 | 1.0 | 0.8 | 帕鲁升级速度 |
| **玩家伤害倍率** | 1.5 | 1.0 | 0.7 | 玩家造成伤害 |
| **玩家承伤倍率** | 0.7 | 1.0 | 1.5 | 玩家受到伤害（>1 承受更多） |
| **帕鲁伤害倍率** | 1.2 | 1.0 | 1.2 | 帕鲁造成伤害 |
| **帕鲁承伤倍率** | 0.7 | 1.0 | 1.5 | 帕鲁受到伤害（>1 承受更多） |
| **玩家耐力降低倍率** | 0.7 | 1.0 | 1.3 | 越低越耐跑 |
| **帕鲁耐力降低倍率** | 0.7 | 1.0 | 1.3 | 帕鲁工作/战斗耐力消耗 |
| **玩家饱食度降低倍率** | 0.5 | 1.0 | 1.5 | 饿肚子速度 |
| **帕鲁饱食度降低倍率** | 0.5 | 1.0 | 1.3 | 帕鲁进食频率 |
| **帕鲁血量降低倍率** | 0.5 | 1.0 | 1.3 | 帕鲁在据点血量流失 |
| **帕鲁精神值降低倍率** | 0.3 | 0.7 | 1.2 | SAN 值下降速度 |
| **捕获倍率** | 2.0 | 1.0 | 0.7 | 帕鲁捕获成功率 |
| **帕鲁生成倍率** | 1.5 | 1.0 | 1.0 | 野外帕鲁数量 |
| **道具掉落倍率** | 1.5 | 1.0 | 0.8 | 击杀掉落数量 |
| **道具重量倍率** | 0.5 | 1.0 | 1.3 | 越低负重压力越小 |
| **蛋孵化时间倍率** | 0.0 | 1.0 | 2.0 | 巨大蛋孵化耗时（0=秒孵） |
| **建造耗材倍率** | 0.5 | 1.0 | 1.3 | 建筑消耗资源 |
| **建造帕鲁工作速度** | 1.5 | 1.0 | 0.8 | 帕鲁干活效率 |
| **建造帕鲁精神值扣减** | 0.5 | 1.0 | 1.3 | 帕鲁劳损速度 |
| **白天速度倍率** | 1.2 | 1.0 | 0.8 | 白天流逝速度 |
| **夜晚速度倍率** | 0.8 | 1.0 | 1.3 | 夜晚流逝速度（>1 更快） |
| **据点袭击频率** | 关闭 | 正常 | 正常+强袭击 | 敌人来袭强度 |
| **偷猎者刷新** | 关闭 | 开启 | 开启+强化 | 野外恶意袭击 |
| **最大据点数量** | 10 | 4 | 3 | 可建据点数量 |
| **工作帕鲁最大数量** | 50 | 25 | 20 | 单据点帕鲁上限 |
| **好友伤害** | False | False | True | PVP 开关 |
| **飞行帕鲁耐力消耗** | 0.3 | 1.0 | 1.5 | 飞行探图爽度 |

---

## 模式一：休闲档 🌿（护肝爽玩）

### 定位
适合**首次入坑**、**下班摸鱼**、**只想欣赏内容/剧情**的玩家。核心思路：**降低时间成本，保留玩法框架**。

### 基础难度与时间配置

| 参数 | 数值 | 说明 |
|------|------|------|
| `Difficulty` | None | 使用自定义配置 |
| `DayTimeSpeedRate` | 1.2 | 白天流逝加快20% |
| `NightTimeSpeedRate` | 0.8 | 夜晚流逝减慢20% |

### 经验与捕获配置

| 参数 | 数值 | 说明 |
|------|------|------|
| `ExpRate` | 1.8 | 经验值获取增加80% |
| `PalCaptureRate` | 2.0 | 帕鲁捕获成功率翻倍 |

### 帕鲁相关配置

| 参数 | 数值 | 说明 |
|------|------|------|
| `PalSpawnNumRate` | 1.5 | 帕鲁出现数量增加50% |
| `PalDamageRateAttack` | 1.2 | 帕鲁攻击伤害增加20% |
| `PalDamageRateDefense` | 0.7 | 帕鲁承受伤害降低30% |
| `PalStomachDecreaceRate` | 0.5 | 帕鲁饱食度消耗减半 |
| `PalStaminaDecreaceRate` | 0.7 | 帕鲁耐力消耗降低30% |
| `PalAutoHPRegeneRate` | 1.0 | 帕鲁生命自然恢复倍率 |
| `PalAutoHpRegeneRateInSleep` | 1.0 | 帕鲁睡眠生命恢复倍率 |

### 玩家相关配置

| 参数 | 数值 | 说明 |
|------|------|------|
| `PlayerDamageRateAttack` | 1.5 | 玩家攻击伤害增加50% |
| `PlayerDamageRateDefense` | 0.7 | 玩家承受伤害降低30% |
| `PlayerStomachDecreaceRate` | 0.5 | 玩家饱食度消耗减半 |
| `PlayerStaminaDecreaceRate` | 0.7 | 玩家耐力消耗降低30% |
| `PlayerAutoHPRegeneRate` | 1.0 | 玩家生命自然恢复倍率 |
| `PlayerAutoHpRegeneRateInSleep` | 1.0 | 玩家睡眠生命恢复倍率 |

### 建筑与采集配置

| 参数 | 数值 | 说明 |
|------|------|------|
| `BuildObjectDamageRate` | 1.0 | 建筑伤害倍率 |
| `BuildObjectHpRate` | 1.0 | 建筑耐久倍率 |
| `BuildObjectDeteriorationDamageRate` | 0.5 | 建筑劣化速度降低50% |
| `CollectionDropRate` | 1.5 | 采集物掉落增加50% |
| `CollectionObjectHpRate` | 1.0 | 采集物生命值倍率 |
| `CollectionObjectRespawnSpeedRate` | 1.0 | 采集物刷新间隔倍率 |

### 物品与战斗配置

| 参数 | 数值 | 说明 |
|------|------|------|
| `EnemyDropItemRate` | 1.5 | 敌人掉落物品增加50% |
| `EquipmentDurabilityDamageRate` | 1.0 | 装备耐久损耗倍率 |
| `ItemWeightRate` | 0.5 | 物品重量减半 |
| `DeathPenalty` | None | 死亡时不掉落任何物品 |
| `bEnablePlayerToPlayerDamage` | False | 关闭玩家对战 |
| `bEnableFriendlyFire` | False | 关闭友军伤害 |
| `bEnableInvaderEnemy` | False | 关闭基地袭击事件 |

### 据点与孵化配置

| 参数 | 数值 | 说明 |
|------|------|------|
| `BaseCampMaxNum` | 10 | 据点总数量上限 |
| `BaseCampMaxNumInGuild` | 10 | 每个公会据点数量上限 |
| `BaseCampWorkerMaxNum` | 50 | 据点帕鲁最大工作数量 |
| `PalEggDefaultHatchingTime` | 0.0 | 蛋秒孵 |

### 完整配置示例

```ini
[/Script/Pal.PalGameWorldSettings]
OptionSettings=(Difficulty=None,DayTimeSpeedRate=1.200000,NightTimeSpeedRate=0.800000,ExpRate=1.800000,PalCaptureRate=2.000000,PalSpawnNumRate=1.500000,PalDamageRateAttack=1.200000,PalDamageRateDefense=0.700000,PlayerDamageRateAttack=1.500000,PlayerDamageRateDefense=0.700000,PlayerStomachDecreaceRate=0.500000,PlayerStaminaDecreaceRate=0.700000,PlayerAutoHPRegeneRate=1.000000,PlayerAutoHpRegeneRateInSleep=1.000000,PalStomachDecreaceRate=0.500000,PalStaminaDecreaceRate=0.700000,PalAutoHPRegeneRate=1.000000,PalAutoHpRegeneRateInSleep=1.000000,BuildObjectDamageRate=1.000000,BuildObjectHpRate=1.000000,BuildObjectDeteriorationDamageRate=0.500000,CollectionDropRate=1.500000,CollectionObjectHpRate=1.000000,CollectionObjectRespawnSpeedRate=1.000000,EnemyDropItemRate=1.500000,EquipmentDurabilityDamageRate=1.000000,ItemWeightRate=0.500000,DeathPenalty=None,bEnablePlayerToPlayerDamage=False,bEnableFriendlyFire=False,bEnableInvaderEnemy=False,BaseCampMaxNum=10,BaseCampMaxNumInGuild=10,BaseCampWorkerMaxNum=50,PalEggDefaultHatchingTime=0.000000)
```

### 体验特点

- 经验值增加80%，快速升级解锁全科技树
- 帕鲁捕获成功率翻倍，一球一个不坐牢
- 玩家攻击伤害增加50%，战斗更轻松
- 玩家承受伤害降低30%，生存更安全
- 饱食度消耗减半，减少资源管理压力
- 白天更长、夜晚更短，减少危险时间
- 蛋秒孵，免去数十分钟干等
- 道具重量减半，背包不爆仓
- 关闭据点袭击，无骚扰专心种田

---

## 模式二：正常档 ⚖️（原汁原味微调）

### 定位
Pocketpair **原本设计意图**的体验，仅做**极小舒适度调整**（不动核心平衡）。适合想认真体验游戏难度曲线、探索感和成就感的玩家。

### 基础难度与时间配置

| 参数 | 数值 | 说明 |
|------|------|------|
| `Difficulty` | None | 使用自定义配置 |
| `DayTimeSpeedRate` | 1.0 | 标准白天时间 |
| `NightTimeSpeedRate` | 1.0 | 标准夜晚时间 |

### 经验与捕获配置

| 参数 | 数值 | 说明 |
|------|------|------|
| `ExpRate` | 1.2 | 经验值获取增加20% |
| `PalCaptureRate` | 1.0 | 标准帕鲁捕获难度 |

### 帕鲁相关配置

| 参数 | 数值 | 说明 |
|------|------|------|
| `PalSpawnNumRate` | 1.0 | 标准帕鲁出现数量 |
| `PalDamageRateAttack` | 1.0 | 标准帕鲁攻击伤害 |
| `PalDamageRateDefense` | 1.0 | 标准帕鲁承受伤害 |
| `PalStomachDecreaceRate` | 1.0 | 标准帕鲁饱食度消耗 |
| `PalStaminaDecreaceRate` | 1.0 | 标准帕鲁耐力消耗 |
| `PalAutoHPRegeneRate` | 1.0 | 标准帕鲁生命恢复 |
| `PalAutoHpRegeneRateInSleep` | 1.0 | 标准帕鲁睡眠恢复 |

### 玩家相关配置

| 参数 | 数值 | 说明 |
|------|------|------|
| `PlayerDamageRateAttack` | 1.0 | 标准玩家攻击伤害 |
| `PlayerDamageRateDefense` | 1.0 | 标准玩家防御能力 |
| `PlayerStomachDecreaceRate` | 1.0 | 标准玩家饱食度消耗 |
| `PlayerStaminaDecreaceRate` | 1.0 | 标准玩家耐力消耗 |
| `PlayerAutoHPRegeneRate` | 1.0 | 标准玩家生命恢复 |
| `PlayerAutoHpRegeneRateInSleep` | 1.0 | 标准玩家睡眠恢复 |

### 建筑与采集配置

| 参数 | 数值 | 说明 |
|------|------|------|
| `BuildObjectDamageRate` | 1.0 | 标准建筑伤害倍率 |
| `BuildObjectHpRate` | 1.0 | 标准建筑耐久倍率 |
| `BuildObjectDeteriorationDamageRate` | 1.0 | 标准建筑劣化速度 |
| `CollectionDropRate` | 1.0 | 标准资源掉落数量 |
| `CollectionObjectHpRate` | 1.0 | 标准采集物生命值 |
| `CollectionObjectRespawnSpeedRate` | 1.0 | 标准采集物刷新间隔 |

### 物品与战斗配置

| 参数 | 数值 | 说明 |
|------|------|------|
| `EnemyDropItemRate` | 1.0 | 标准敌人掉落数量 |
| `EquipmentDurabilityDamageRate` | 1.0 | 标准装备耐久损耗 |
| `ItemWeightRate` | 1.0 | 标准物品重量 |
| `DeathPenalty` | ItemAndEquipment | 死亡掉落物品和装备 |
| `bEnablePlayerToPlayerDamage` | False | 关闭玩家对战 |
| `bEnableFriendlyFire` | False | 关闭友军伤害 |
| `bEnableInvaderEnemy` | True | 启用基地袭击事件 |

### 据点与孵化配置

| 参数 | 数值 | 说明 |
|------|------|------|
| `BaseCampMaxNum` | 4 | 据点总数量上限（比默认多1个） |
| `BaseCampMaxNumInGuild` | 4 | 每个公会据点数量上限 |
| `BaseCampWorkerMaxNum` | 25 | 据点帕鲁最大工作数量 |
| `PalEggDefaultHatchingTime` | 1.0 | 标准蛋孵化时间 |

### 完整配置示例

```ini
[/Script/Pal.PalGameWorldSettings]
OptionSettings=(Difficulty=None,DayTimeSpeedRate=1.000000,NightTimeSpeedRate=1.000000,ExpRate=1.200000,PalCaptureRate=1.000000,PalSpawnNumRate=1.000000,PalDamageRateAttack=1.000000,PalDamageRateDefense=1.000000,PlayerDamageRateAttack=1.000000,PlayerDamageRateDefense=1.000000,PlayerStomachDecreaceRate=1.000000,PlayerStaminaDecreaceRate=1.000000,PlayerAutoHPRegeneRate=1.000000,PlayerAutoHpRegeneRateInSleep=1.000000,PalStomachDecreaceRate=1.000000,PalStaminaDecreaceRate=1.000000,PalAutoHPRegeneRate=1.000000,PalAutoHpRegeneRateInSleep=1.000000,BuildObjectDamageRate=1.000000,BuildObjectHpRate=1.000000,BuildObjectDeteriorationDamageRate=1.000000,CollectionDropRate=1.000000,CollectionObjectHpRate=1.000000,CollectionObjectRespawnSpeedRate=1.000000,EnemyDropItemRate=1.000000,EquipmentDurabilityDamageRate=1.000000,ItemWeightRate=1.000000,DeathPenalty=ItemAndEquipment,bEnablePlayerToPlayerDamage=False,bEnableFriendlyFire=False,bEnableInvaderEnemy=True,BaseCampMaxNum=4,BaseCampMaxNumInGuild=4,BaseCampWorkerMaxNum=25,PalEggDefaultHatchingTime=1.000000)
```

### 体验特点

- 经验值增加20%，略微加快前中期进度
- 保留完整的游戏挑战和平衡
- 死亡掉落物品和装备，但保留帕鲁
- 据点袭击开启，保留完整威胁系统
- 捕获率标准，越级抓boss有真实失败风险
- 据点数量4个，覆盖矿场/农场/繁殖场/主基地

---

## 模式三：困难档 🔥（硬核挑战）

### 定位
让 Palworld 变回**真正的"生存"游戏**。适合已经通关一次、想追求挑战、或喜欢生存黑魂类节奏的玩家。

### 基础难度与时间配置

| 参数 | 数值 | 说明 |
|------|------|------|
| `Difficulty` | None | 使用自定义配置 |
| `DayTimeSpeedRate` | 0.8 | 白天流逝减慢20% |
| `NightTimeSpeedRate` | 1.3 | 夜晚流逝加快30% |

### 经验与捕获配置

| 参数 | 数值 | 说明 |
|------|------|------|
| `ExpRate` | 0.8 | 经验值获取减少20% |
| `PalCaptureRate` | 0.7 | 帕鲁捕获成功率降低30% |

### 帕鲁相关配置

| 参数 | 数值 | 说明 |
|------|------|------|
| `PalSpawnNumRate` | 1.0 | 标准帕鲁出现数量 |
| `PalDamageRateAttack` | 1.2 | 帕鲁攻击伤害增加20% |
| `PalDamageRateDefense` | 1.5 | 帕鲁承受伤害增加50% |
| `PalStomachDecreaceRate` | 1.3 | 帕鲁饱食度消耗加快30% |
| `PalStaminaDecreaceRate` | 1.3 | 帕鲁耐力消耗加快30% |
| `PalAutoHPRegeneRate` | 1.0 | 标准帕鲁生命恢复 |
| `PalAutoHpRegeneRateInSleep` | 1.0 | 标准帕鲁睡眠恢复 |

### 玩家相关配置

| 参数 | 数值 | 说明 |
|------|------|------|
| `PlayerDamageRateAttack` | 0.7 | 玩家攻击伤害减少30% |
| `PlayerDamageRateDefense` | 1.5 | 玩家承受伤害增加50% |
| `PlayerStomachDecreaceRate` | 1.5 | 玩家饱食度消耗加快50% |
| `PlayerStaminaDecreaceRate` | 1.3 | 玩家耐力消耗加快30% |
| `PlayerAutoHPRegeneRate` | 1.0 | 标准玩家生命恢复 |
| `PlayerAutoHpRegeneRateInSleep` | 1.0 | 标准玩家睡眠恢复 |

### 建筑与采集配置

| 参数 | 数值 | 说明 |
|------|------|------|
| `BuildObjectDamageRate` | 1.0 | 标准建筑伤害倍率 |
| `BuildObjectHpRate` | 1.0 | 标准建筑耐久倍率 |
| `BuildObjectDeteriorationDamageRate` | 1.3 | 建筑劣化速度加快30% |
| `CollectionDropRate` | 0.8 | 采集物掉落减少20% |
| `CollectionObjectHpRate` | 1.0 | 标准采集物生命值 |
| `CollectionObjectRespawnSpeedRate` | 1.0 | 标准采集物刷新间隔 |

### 物品与战斗配置

| 参数 | 数值 | 说明 |
|------|------|------|
| `EnemyDropItemRate` | 0.8 | 敌人掉落物品减少20% |
| `EquipmentDurabilityDamageRate` | 1.0 | 标准装备耐久损耗 |
| `ItemWeightRate` | 1.3 | 物品重量增加30% |
| `DeathPenalty` | All | 死亡掉落所有物品、装备和帕鲁 |
| `bEnablePlayerToPlayerDamage` | False | 关闭玩家对战 |
| `bEnableFriendlyFire` | True | 开启友军伤害（多人挑战） |
| `bEnableInvaderEnemy` | True | 启用基地袭击事件 |

### 据点与孵化配置

| 参数 | 数值 | 说明 |
|------|------|------|
| `BaseCampMaxNum` | 3 | 据点总数量上限（比默认少1个） |
| `BaseCampMaxNumInGuild` | 3 | 每个公会据点数量上限 |
| `BaseCampWorkerMaxNum` | 20 | 据点帕鲁最大工作数量 |
| `PalEggDefaultHatchingTime` | 2.0 | 蛋孵化时间翻倍 |

### 完整配置示例

```ini
[/Script/Pal.PalGameWorldSettings]
OptionSettings=(Difficulty=None,DayTimeSpeedRate=0.800000,NightTimeSpeedRate=1.300000,ExpRate=0.800000,PalCaptureRate=0.700000,PalSpawnNumRate=1.000000,PalDamageRateAttack=1.200000,PalDamageRateDefense=1.500000,PlayerDamageRateAttack=0.700000,PlayerDamageRateDefense=1.500000,PlayerStomachDecreaceRate=1.500000,PlayerStaminaDecreaceRate=1.300000,PlayerAutoHPRegeneRate=1.000000,PlayerAutoHpRegeneRateInSleep=1.000000,PalStomachDecreaceRate=1.300000,PalStaminaDecreaceRate=1.300000,PalAutoHPRegeneRate=1.000000,PalAutoHpRegeneRateInSleep=1.000000,BuildObjectDamageRate=1.000000,BuildObjectHpRate=1.000000,BuildObjectDeteriorationDamageRate=1.300000,CollectionDropRate=0.800000,CollectionObjectHpRate=1.000000,CollectionObjectRespawnSpeedRate=1.000000,EnemyDropItemRate=0.800000,EquipmentDurabilityDamageRate=1.000000,ItemWeightRate=1.300000,DeathPenalty=All,bEnablePlayerToPlayerDamage=False,bEnableFriendlyFire=True,bEnableInvaderEnemy=True,BaseCampMaxNum=3,BaseCampMaxNumInGuild=3,BaseCampWorkerMaxNum=20,PalEggDefaultHatchingTime=2.000000)
```

### 体验特点

- 经验值减少20%，升级更慢
- 捕获率降低30%，boss一球捉基本不可能
- 玩家攻击伤害减少30%，战斗效率降低
- 玩家承受伤害增加50%，一次失误可能团灭
- 饱食度消耗加快30-50%，需要精细管理
- 夜晚更长，增加生存压力和危险
- 资源掉落减少20%，资源管理成为关键
- 死亡掉落所有物品和帕鲁，代价巨大
- 据点数量限制为3个，需要策略性选择

---

## 关键注意事项

### 1. 成就解锁限制
- 单人模式下，Steam 成就在**难度低于普通**时会被禁用（休闲档大部分成就无法解锁）
- 困难档所有成就正常解锁并有专属成就

### 2. 伤害倍率的反直觉设计
- **PlayerDamageRateDefense > 1** = 玩家承受更多伤害（更困难）
- **PlayerDamageRateDefense < 1** = 玩家承受更少伤害（更容易）

### 3. 白天黑夜速度倍率
- **DayTimeSpeedRate > 1** = 白天流逝更快
- **NightTimeSpeedRate > 1** = 夜晚流逝更快
- 困难模式下白天0.8、夜晚1.3，意味着白天更短、夜晚更长

### 4. 蛋孵化时间bug
官方已知bug——预设"休闲"档孵化时间不是0，需要手动设置为0才是真的秒孵

### 5. WorldOption.sav 覆盖问题
一旦创建了世界存档，游戏会生成 `WorldOption.sav` 文件，该文件会覆盖 `PalWorldSettings.ini` 中的部分设置。修改世界级参数后需删除此文件或使用官方工具重置

### 6. 配置文件格式
所有设置必须在同一行内，用逗号分隔，不要添加换行或空格

---

## 快速上手建议

| 你是谁 | 推荐档位 |
|--------|----------|
| 首次入坑、没打过 EA | 休闲档 |
| 打过 EA、想认真体验 1.0 | 正常档 |
| 通关过 1.0、想再挑战 | 困难档 |
| 上班族只有 1-2 小时/晚 | 休闲档 |
| 和女朋友/朋友合作 | 正常档（关闭好友伤害） |
| 想开直播/录视频 | 正常档或困难档 |
| 单纯想拍照/建家党 | 休闲档 + 关闭袭击 |

---

## 参考资料

1. [F:\study\research\palworld\幻兽帕鲁-世界参数三档攻略.md](file:///F:/study/research/palworld/幻兽帕鲁-世界参数三档攻略.md)
2. [F:\study\research\palworld\幻兽帕鲁-游戏基础信息.md](file:///F:/study/research/palworld/幻兽帕鲁-游戏基础信息.md)
3. [F:\study\research\palworld\幻兽帕鲁-实用技巧与进阶玩法.md](file:///F:/study/research/palworld/幻兽帕鲁-实用技巧与进阶玩法.md)
4. [Steam 商店页面 - Palworld](https://store.steampowered.com/app/1623730/Palworld/)
5. [Palworld 官方技术文档 - 配置说明](https://tech.palworldgame.com/settings-and-operation/configuration)
6. [B站 - 幻兽帕鲁正式版世界难度设置](https://www.bilibili.com/video/BV1o2NL69EaE/)
7. [3DMGame - Palworld Server Configuration Guide](https://m.3dmgame.com/gl/3981837_7.html)