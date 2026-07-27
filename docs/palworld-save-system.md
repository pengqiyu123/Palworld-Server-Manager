# 幻兽帕鲁存档系统技术解析

基于 PalworldSaveTools 源码深度分析存档数据结构、文件位置、解析机制、数据内容与迁移流程

**标签**: `Palworld` `UE5 GVAS` `PalworldSaveTools`
**基于源码版本**: PalworldSaveTools-main

## 目录

1. [存档数据结构：从 .sav 到 GVAS](#1-存档数据结构从-sav-到-gvas)
2. [存档文件位置：本地单机与专用服务器](#2-存档文件位置本地单机与专用服务器)
3. [PalworldSaveTools 解析机制：三层管道](#3-palworldsavetools-解析机制三层管道)
4. [存档数据内容：世界与玩家的完整快照](#4-存档数据内容世界与玩家的完整快照)
5. [数据迁移：文件清单与操作流程](#5-数据迁移文件清单与操作流程)

---

## 1. 存档数据结构：从 .sav 到 GVAS

幻兽帕鲁基于 **Unreal Engine 5** 开发，其存档系统使用 UE 引擎原生的 **GVAS（Global Visual Archive Save）** 序列化格式。存档文件以 `.sav` 扩展名存储，但实际内容是经过**压缩**的二进制数据，解压后才是可解析的 GVAS 结构。

### 1.1 .sav 文件二进制布局

每个 `.sav` 文件由一个 **12 字节头部**（CNK 格式为 24 字节）和**压缩数据体**组成。头部结构如下：

| 偏移   | 字节                  | 字段名              | 类型    | 说明           |
| ------ | --------------------- | ------------------- | ------- | -------------- |
| 0x00   | XX XX XX XX           | uncompressed_length | u32 LE  | 解压后总长度   |
| 0x04   | XX XX XX XX           | compressed_length   | u32 LE  | 压缩数据长度   |
| 0x08   | 50 6C 5A (`PlZ`)      | magic_bytes         | 3 bytes | 魔数标识       |
| 0x0B   | 32 (`2`)              | save_type           | u8      | 压缩类型 (50=PLZ) |
| 0x0C   | XX XX XX ...          | compressed_data     | bytes[] | 压缩数据体     |

PalworldSaveTools 的 `compressor/__init__.py` 中 `_parse_sav_header()` 方法精确解析了这个结构：

```python
def _parse_sav_header(self, sav_data: bytes) -> Tuple[int, int, bytes, int, int]:
    uncompressed_len = int.from_bytes(sav_data[0:4], byteorder='little')
    compressed_len = int.from_bytes(sav_data[4:8], byteorder='little')
    magic_bytes = sav_data[8:11]
    save_type = sav_data[11]
    data_offset = 12
    # CNK 格式有额外的 12 字节二级头
    if magic_bytes == MagicBytes.CNK.value:
        # 跳过第一层，读取真正的头
        uncompressed_len = int.from_bytes(sav_data[12:16], byteorder='little')
        compressed_len = int.from_bytes(sav_data[16:20], byteorder='little')
        magic_bytes = sav_data[20:23]
        save_type = sav_data[23]
        data_offset = 24
    return (uncompressed_len, compressed_len, magic_bytes, save_type, data_offset)
```

### 1.2 三种压缩格式

幻兽帕鲁存档使用三种压缩格式，通过魔数和类型字节区分：

| 魔数  | 类型值     | 名称 | 压缩算法       | 使用场景                          |
| ----- | ---------- | ---- | -------------- | --------------------------------- |
| `PlZ` | 50 (0x32)  | PLZ  | 双重 zlib      | **世界存档**（Level.sav）         |
| `PlM` | 49 (0x31)  | PLM  | Oodle (Kraken) | **玩家存档**及其他 .sav 文件      |
| `CNK` | 48 (0x30)  | CNK  | 分块 zlib      | 旧格式 / 兼容场景                 |

> **关键区分逻辑**
>
> PalworldSaveTools 通过检查 `save_game_class_name` 是否包含 `Pal.PalworldSaveGame`（注意小写 `w`）来判断文件类型。世界存档使用 **PLZ（双重 zlib）**，其他存档使用 **PLM（Oodle）**。PLZ 格式需要两次 zlib 解压：第一次解压得到中间数据，第二次解压得到最终 GVAS 字节。

### 1.3 GVAS 文件结构

解压后的数据是标准的 **GVAS 格式**，由 `gvas.py` 中的 `GvasFile` 类解析。GVAS 结构分为三部分：

**图 1：GVAS 文件结构布局**

```
┌─ GvasHeader ────────────────────────────────────────────────┐
│ magic                            i32 = 0x53414756 ("GVAS")   │
│ save_game_version                i32 = 3                     │
│ package_file_version_ue4 / ue5   i32, i32                    │
│ engine_version_{major,minor,patch,changelist,branch}         │
│                                  UE5 版本信息                 │
├─ Properties (UE Property System) ────────────────────────────┤
│ StrProperty           → 字符串值 (玩家名, 世界名等)            │
│ IntProperty/FloatProperty → 数值 (等级, HP, 科技点等)          │
│ ArrayProperty         → 数组 (角色列表, 物品列表等)            │
│ MapProperty           → 键值映射 (CharacterSaveParameterMap 等)│
│ StructProperty        → 结构体 (Vector, Guid, DateTime, 自定义)│
├─ Trailer ────────────────────────────────────────────────────┤
│ 通常为 4 字节 0x00000000，非零则表示解析不完整                  │
│ 若 trailer != b'\x00\x00\x00\x00' → 文件可能未完全解析         │
└──────────────────────────────────────────────────────────────┘
```

### 1.4 UE 属性系统详解

`archive.py` 中的 `FArchiveReader` 类实现了完整的 UE 属性反序列化。每个属性以 **名称 → 类型 → 大小** 的三元组开头，直到遇到 `"None"` 字符串终止：

```python
def properties_until_end(self, path='') -> dict[str, Any]:
    properties = {}
    while True:
        name = self.fstring()       # 属性名 (如 "SaveData", "worldSaveData")
        if name == 'None':          # 属性列表终止标记
            break
        type_name = self.fstring()  # 类型名 (如 "StructProperty", "IntProperty")
        size = self.u64()           # 属性值字节大小
        properties[name] = self.property(type_name, size, f'{path}.{name}')
    return properties
```

属性类型分发器 `_READ_PROPERTY_DISPATCH` 支持以下 16 种 UE 原生属性类型：

| 属性类型              | 读取方法                     | 数据格式                                        |
| --------------------- | ---------------------------- | ----------------------------------------------- |
| `StructProperty`      | `_read_StructProperty`       | 嵌套结构体 (Vector, Guid, DateTime, Quat, 自定义) |
| `IntProperty`         | `_read_IntProperty`          | i32 + optional_guid                             |
| `FloatProperty`       | `_read_FloatProperty`        | f32 + optional_guid                             |
| `StrProperty`         | `_read_StrProperty`          | fstring (ASCII 或 UTF-16-LE)                    |
| `BoolProperty`        | `_read_BoolProperty`         | 1 byte + optional_guid                          |
| `ArrayProperty`       | `_read_ArrayProperty`        | 子类型 + count + 元素列表                       |
| `MapProperty`         | `_read_MapProperty`          | key_type + value_type + 键值对数组              |
| `SetProperty`         | `_read_SetProperty`          | set_type + 元素集合                             |
| `EnumProperty`        | `_read_EnumProperty`         | 枚举类型名 + 枚举值                             |
| `ByteProperty`        | `_read_ByteProperty`         | 枚举类型名 + byte/fstring                       |
| `NameProperty`        | `_read_NameProperty`         | fstring                                         |
| `UInt64Property`      | `_read_UInt64Property`       | u64                                             |
| `Int64Property`       | `_read_Int64Property`        | i64                                             |
| `UInt32Property`      | `_read_UInt32Property`       | u32                                             |
| `UInt16Property`      | `_read_UInt16Property`       | u16                                             |
| `FixedPoint64Property`| `_read_FixedPoint64Property` | i32 (定点数)                                    |

#### fstring 编码规则

UE 的 `fstring` 使用**长度前缀**编码，正长度表示 ASCII，负长度表示 UTF-16-LE：

```python
def fstring(self) -> str:
    size = self.i32()            # 读取 4 字节长度前缀
    if size == 0:
        return ''
    if size < 0:                 # 负数 → UTF-16-LE 编码
        size = -size
        data = self.data.read(size * 2)[:-2]  # 去掉末尾 \x00\x00
        return data.decode('utf-16-le')
    else:                        # 正数 → ASCII 编码
        data = self.data.read(size)[:-1]      # 去掉末尾 \x00
        return data.decode('ascii')
```

#### UUID 字节序

Palworld 的 UUID/GUID 使用**混合字节序**存储，并非简单的 little-endian。前 3 组使用小端序，第 4-5 组使用大端序：

```python
# archive.py 中的 UUID 字节重排逻辑
# 原始字节: [b0, b1, b2, b3, b4, b5, b6, b7, b8, b9, b10, b11, b12, b13, b14, b15]
# 格式化:   %08x-%04x-%04x-%04x-%04x%08x
#           b3b2b1b0-b7b6-b5b4-b11b10-b9b8-b15b14b13b12

def __str__(self) -> str:
    b = self.raw_bytes
    return '%08x-%04x-%04x-%04x-%04x%08x' % (
        b[3] << 24 | b[2] << 16 | b[1] << 8 | b[0],   # 前4字节小端
        b[7] << 8 | b[6],                             # 第5-6字节小端
        b[5] << 8 | b[4],                             # 第7-8字节小端
        b[11] << 8 | b[10],                           # 第9-10字节大端
        b[9] << 8 | b[8],                             # 第11-12字节大端
        b[15] << 24 | b[14] << 16 | b[13] << 8 | b[12] # 后4字节大端
    )
```

---

## 2. 存档文件位置：本地单机与专用服务器

### 2.1 本地单机 / 联机主机存档

本地存档（包括单人模式和联机主机）存储在 Windows 的 `AppData\Local` 目录下：

```
C:\Users\<用户名>\AppData\Local\Pal\Saved\SaveGames\
├── <SteamID>/                        # 你的 Steam 数字 ID
│   ├── <WorldID>/                    # 世界唯一标识 (十六进制字符串)
│   │   ├── Level.sav                 # 世界存档 (所有世界数据, PLZ 压缩) ★
│   │   ├── LevelMeta.sav             # 世界元数据 (世界名, 世界选项)
│   │   ├── WorldOption.sav           # 世界设置 (难度, 参数等)
│   │   └── Players/                  # 玩家存档文件夹
│   │       ├── 0001.sav              # 主机玩家存档 (固定文件名) ★
│   │       ├── <SteamID>.sav         # 联机客户端玩家存档
│   │       └── <SteamID>_dps.sav     # 玩家帕鲁仓库 (Dynamic Player Storage)
│   └── backup/                       # 游戏自动备份
│       ├── local/                    # 本地存档备份
│       └── world/                    # 世界存档备份
```

### 2.2 专用服务器存档

专用服务器的存档路径结构类似，但有两个关键区别：

```
<Steam库>\steamapps\common\PalServer\Pal\Saved\SaveGames\
├── 0/                                # 服务器固定使用 "0" 代替 SteamID
│   └── <ServerWorldID>/              # 服务器世界唯一标识
│       ├── Level.sav                 # 世界存档 (同本地) ★
│       ├── LevelMeta.sav             # 世界元数据
│       ├── WorldOption.sav           # 世界设置
│       └── Players/
│           ├── <PlayerUID>.sav       # 每个玩家的存档 (无固定 0001.sav)
│           └── <PlayerUID>_dps.sav   # 玩家帕鲁仓库
```

### 2.3 本地与服务器的关键差异

| 维度         | 本地单机 / 联机主机                        | 专用服务器                                |
| ------------ | ------------------------------------------ | ----------------------------------------- |
| 用户 ID 目录 | `<SteamID>`（如 `765611980xxx`）           | 固定为 `0`                                |
| 主机存档     | `0001.sav`（固定文件名）                   | 无固定主机文件，所有玩家平等              |
| 玩家标识     | Steam ID 转换的 UID                        | 首次连接时分配的 PlayerUID                |
| 存档根路径   | `%localappdata%\Pal\Saved\SaveGames\`      | `PalServer\Pal\Saved\SaveGames\`          |
| 配置文件位置 | `%localappdata%\Pal\Saved\Config\WindowsServer\` | `PalServer\Pal\Saved\Config\WindowsServer\` |

> **注意**
>
> 服务器的 `GameUserSettings.ini` 中的 `DedicatedServerName` 字段必须与 `SaveGames\0\<ServerWorldID>` 目录名一致，否则服务器会创建新世界。迁移存档时如果世界 ID 变化，需要同步修改此配置。

### 2.4 其他平台存档位置

#### Xbox Game Pass / Microsoft Store

XGP 版本的存档存储在 Windows 的容器虚拟化目录中，路径较深且不直观：

```
C:\Users\<用户名>\AppData\Local\Packages\
└── PocketpairInc.Palworld_ab9nevgr8w1tm\SystemAppData\wgs\
    └── <ContainerID>/
        └── <SaveID>/
            ├── container*            # XGP 容器元数据
            └── (存档文件, 无 .sav 扩展名)
```

PalworldSaveTools 提供了 `xgp_save_extract.py` 和 `gamepass_manager.py` 模块，专门用于从 XGP 容器中提取和导入存档。

---

## 3. PalworldSaveTools 解析机制：三层管道

PalworldSaveTools 的核心设计是 **SAV ↔ GVAS ↔ JSON** 三层转换管道 <sup>[[1]](#cite-1)</sup>。这个设计将"压缩/解压"、"序列化/反序列化"、"原始数据解析"三个关注点完全分离，每一层可以独立替换或测试。

**图 2：PalworldSaveTools 三层解析管道架构**

```
┌─ Layer 1: SAV ↔ Raw GVAS (压缩层) ──────────────────────────────────┐
│ core.py → compressor/__init__.py → zlib.py / oozlib.py               │
│                                                                      │
│ [.sav 文件] → _parse_sav_header() → zlib.decompress() x2 → raw GVAS bytes
└────────────────────────────────┬─────────────────────────────────────┘
                                 ▼
┌─ Layer 2: Raw GVAS ↔ JSON Properties (序列化层) ────────────────────┐
│ gvas.py → archive.py (FArchiveReader/Writer)                         │
│                                                                      │
│ [raw GVAS bytes] → GvasHeader.read() → properties_until_end() → JSON dict
└────────────────────────────────┬─────────────────────────────────────┘
                                 ▼
┌─ Layer 3: RawData ↔ Structured JSON (自定义解码层) ─────────────────┐
│ paltypes.py (PALWORLD_CUSTOM_PROPERTIES) → rawdata/*.py              │
│                                                                      │
│ character.py, base_camp.py, item_container.py, group.py,             │
│ map_object.py, foliage_model.py, work.py ...                         │
└──────────────────────────────────────────────────────────────────────┘
```

### 3.1 第一层：SAV 解压 (core.py + compressor/)

`io.py` 的 `load_sav()` 是入口函数，读取 .sav 文件并调用 `core.py` 解压：

```python
# io.py — 入口函数
def load_sav(path, type_hints=None, custom_properties=None, ...) -> 'GvasFile':
    with open(path, 'rb') as f:
        data = f.read()
    raw_gvas, _ = decompress_sav_to_gvas(data)  # 调用 core.py
    return GvasFile.read(raw_gvas, type_hints, custom_properties)

# core.py — 格式路由
def decompress_sav_to_gvas(data: bytes, debug=False) -> Tuple[bytes, int]:
    format = compressor.check_sav_format(data)   # 检查魔数
    match format:
        case SaveType.PLZ | SaveType.CNK:
            return z_lib.decompress(data)       # zlib 双重解压
        case SaveType.PLM:
            return oozlib.decompress(data)      # Oodle Kraken 解压
```

**PLZ 解压流程**（双重 zlib）：读取头部 → 第一次 zlib.decompress → 验证 compressed_len → 第二次 zlib.decompress → 验证 uncompressed_len

**PLM 解压流程**（Oodle）：读取头部 → 调用 `palooz.decompress()`（原生 Oodle Kraken 库）→ 验证解压长度

### 3.2 第二层：GVAS 反序列化 (gvas.py + archive.py)

解压后的 raw GVAS 字节由 `GvasFile.read()` 解析。这个过程依赖 `FArchiveReader` 提供的**二进制读取原语**：

```python
# gvas.py — GVAS 文件读取
class GvasFile:
    @staticmethod
    def read(data: bytes, type_hints={}, custom_properties={}) -> 'GvasFile':
        gvas = GvasFile()
        with FArchiveReader(data, type_hints=type_hints,
                           custom_properties=custom_properties) as reader:
            gvas.header = GvasHeader.read(reader)        # 1. 读取 GVAS 头
            gvas.properties = reader.properties_until_end()  # 2. 读取属性字典
            gvas.trailer = reader.read_to_end()          # 3. 读取尾部
        return gvas
```

`FArchiveReader` 提供的关键读取原语：

| 方法                | 读取字节数 | 用途                                   |
| ------------------- | ---------- | -------------------------------------- |
| `i32() / u32()`     | 4          | 32 位整数（长度前缀、计数值）          |
| `u64() / i64()`     | 8          | 64 位整数（属性大小、时间戳）          |
| `u16() / i16()`     | 2          | 16 位整数（引擎版本号）                |
| `float() / double()`| 4 / 8      | 浮点数（坐标、属性值）                 |
| `byte() / bool()`   | 1          | 字节/布尔值                            |
| `fstring()`         | 可变       | UE 字符串（长度前缀 + ASCII/UTF-16）   |
| `guid()`            | 16         | UUID/GUID（混合字节序）                |
| `optional_guid()`   | 1+16       | 可选 GUID（1 字节标志 + 可选 16 字节） |
| `tarray(reader)`    | 可变       | UE TArray（u32 计数 + 元素列表）       |
| `vector() / quat()` | 24 / 32    | 3D 向量 / 四元数（3 或 4 个 double）   |

### 3.3 第三层：自定义原始数据解码 (paltypes.py + rawdata/)

Palworld 在标准 UE 属性系统之上，使用 **RawData** 字段存储复杂的游戏特定数据。这些字段以 `ArrayProperty<ByteProperty>` 形式存在，内容是自定义二进制格式，需要专用解码器。

`paltypes.py` 通过两个字典配置解码行为：

#### PALWORLD_TYPE_HINTS

指定 `StructProperty` 的具体类型。UE 的 StructProperty 本身不携带类型信息，PalworldSaveTools 需要通过路径提示来推断：

```python
PALWORLD_TYPE_HINTS = {
    '.worldSaveData.CharacterSaveParameterMap.Key': 'StructProperty',
    '.worldSaveData.BaseCampSaveData.Key': 'Guid',
    '.worldSaveData.GroupSaveDataMap.Key': 'Guid',
    # ... 40+ 条类型提示
}
```

#### PALWORLD_CUSTOM_PROPERTIES

为特定路径注册自定义 (decoder, encoder) 函数对。当 `FArchiveReader` 遇到注册的路径时，会调用自定义解码器而非默认的属性读取逻辑：

```python
PALWORLD_CUSTOM_PROPERTIES = {
    '.worldSaveData.GroupSaveDataMap':           (group.decode, group.encode),
    '.worldSaveData.CharacterSaveParameterMap.Value.RawData': (character.decode, character.encode),
    '.worldSaveData.ItemContainerSaveData.Value.RawData':     (item_container.decode, item_container.encode),
    '.worldSaveData.BaseCampSaveData.Value.RawData':          (base_camp.decode, base_camp.encode),
    '.worldSaveData.MapObjectSaveData':                  (map_object.decode, map_object.encode),
    '.worldSaveData.FoliageGridSaveDataMap.Value.ModelMap.Value.RawData': (foliage_model.decode, foliage_model.encode),
    # ... 17 条自定义解码器
}
```

#### 原始数据解码示例 (character.py)

角色/帕鲁的原始数据解码是最核心的自定义解码器之一：

```python
# rawdata/character.py
def decode_bytes(parent_reader, char_bytes) -> dict:
    reader = parent_reader.internal_copy(bytes(char_bytes), debug=False)
    char_data = {
        'object': reader.properties_until_end(),  # SaveParameter 属性
        'unknown_bytes': reader.byte_list(4),     # 4 字节未知数据
        'group_id': reader.guid(),                # 所属公会 GUID
    }
    char_data['trailing_bytes'] = reader.byte_list(4)
    if not reader.eof():
        char_data['trailing_unknown_bytes'] = reader.read_to_end()
    return char_data
```

> **GUI 与 CLI 的解码差异**
>
> GUI 模式使用 `SKP_PALWORLD_CUSTOM_PROPERTIES`，其中 6 个路径被设为 no-op（跳过解码）以提升速度。完整的 foliage/spawner 编辑需要使用 CLI 模式。这是 `palobject.py` 中的性能优化策略。

---

## 4. 存档数据内容：世界与玩家的完整快照

### 4.1 Level.sav — 世界存档数据结构

`Level.sav` 是最大的存档文件，包含整个世界的所有数据。其 GVAS 属性 `worldSaveData` 下的子属性构成了完整的世界状态：

| 属性名                          | 数据类型       | 内容描述                                                                                                                          |
| ------------------------------- | -------------- | --------------------------------------------------------------------------------------------------------------------------------- |
| `CharacterSaveParameterMap`     | MapProperty    | **所有角色数据**（玩家 + 帕鲁），以 (PlayerUId, InstanceId) 为键。每个条目的 RawData 包含完整属性：等级、HP、攻击、防御、技能、被动、工作适应性、主人 UID 等 |
| `CharacterContainerSaveData`    | MapProperty    | **帕鲁容器**（帕鲁盒 / DPS 仓库），记录每个容器中的帕鲁槽位分配                                                                    |
| `ItemContainerSaveData`         | MapProperty    | **物品容器**（箱子、玩家背包、装备栏），每个容器的槽位和物品 ID/数量                                                               |
| `MapObjectSaveData`             | ArrayProperty  | **建筑/结构**数据，包括建筑坐标、旋转、模块信息、耐久度                                                                            |
| `BaseCampSaveData`              | MapProperty    | **据点/营地**数据：据点位置、半径、工作分配、工人帕鲁列表、工作集合                                                                |
| `GroupSaveDataMap`              | MapProperty    | **公会**数据：公会名、等级、会长、成员列表、公会仓库、实验室研究进度                                                               |
| `FoliageGridSaveDataMap`        | MapProperty    | **植被**状态：树木砍伐记录、资源采集状态（按网格分区）                                                                             |
| `MapObjectSpawnerInStageSaveData` | MapProperty  | **怪物/物品生成器**状态：每个生成器的阶段、掉落物记录                                                                              |
| `WorkSaveData`                  | StructProperty | **工作分配**数据：帕鲁工作指派、工作进度                                                                                           |
| `DungeonSaveData`               | StructProperty | **地下城**状态：通关记录、奖励宝箱、内部建筑                                                                                       |
| `EnemyCampSaveData`             | StructProperty | **敌方营地**状态：营地攻略进度、宝箱                                                                                               |
| `InvaderSaveData`               | MapProperty    | **入侵者事件**状态                                                                                                                 |
| `OilrigSaveData`                | StructProperty | **石油平台**攻略状态                                                                                                               |
| `SupplySaveData`                | StructProperty | **空投**状态记录                                                                                                                   |
| `GuildExtraSaveDataMap`         | MapProperty    | **公会扩展**数据：公会共享仓库、实验室研究                                                                                         |
| `GameTimeSaveData`              | StructProperty | **游戏时间**：当前世界时间戳（用于计算玩家最后在线时间）                                                                           |
| `DynamicItemSaveData`           | StructProperty | **动态物品**：地面上掉落的物品实体                                                                                                 |

### 4.2 玩家存档文件结构

每个玩家的 `.sav` 文件（如 `0001.sav`）包含该玩家的个人数据：

```json
{
  "header": { ... },
  "properties": {
    "SaveData": {
      "type": "StructProperty",
      "value": {
        "PlayerUId": { "value": "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx" },
        "IndividualId": {
          "value": {
            "PlayerUId": { "value": "同上" },
            "InstanceId": { "value": "角色实例 GUID" }
          }
        },
        "PalStorageContainerId": { "value": { "ID": { "value": "帕鲁仓库容器 ID" } } }
        // ... 玩家科技点、地图解锁、快速旅行点、物品栏等
      }
    }
  }
}
```

### 4.3 _dps.sav — 帕鲁仓库文件

`<UID>_dps.sav` 文件存储玩家的**帕鲁仓库**（Dynamic Player Storage），即帕鲁盒中的帕鲁数据：

```json
{
  "properties": {
    "SaveParameterArray": {
      "type": "ArrayProperty",
      "value": {
        "values": [
          {
            "SaveParameter": {
              "struct_type": "PalIndividualCharacterSaveParameter",
              "value": {
                "SlotId": { "value": { "ContainerId": { ... }, "SlotIndex": 0 } },
                "OwnerPlayerUId": { "value": "玩家 UID" },
                "CharacterID": { "value": "Pal_ID" },
                "Level": { "value": 50 },
                "HP": { "value": 5000 },
                "Attack": { "value": 1200 }
                // IVs, Souls, Skills, Passives, Rank, WorkSuitability ...
              }
            }
          }
          // ... 更多帕鲁
        ]
      }
    }
  }
}
```

### 4.4 帕鲁数据完整字段

每个帕鲁的 `PalIndividualCharacterSaveParameter` 结构包含以下核心字段：

| 字段名                                       | 类型                | 说明                                  |
| -------------------------------------------- | ------------------- | ------------------------------------- |
| `CharacterID`                                | StrProperty         | 帕鲁种类 ID（如 `Lamball`）           |
| `Level`                                      | IntProperty         | 当前等级 (1-80)                       |
| `HP / MaxHP`                                 | IntProperty         | 当前/最大生命值                       |
| `Attack / Defense`                           | IntProperty         | 攻击/防御力                           |
| `TalentHP / TalentAttack / TalentDefense`    | IntProperty         | 个体值 IV (0-100)                     |
| `Rank`                                       | IntProperty         | 强化等级 (0-4)                        |
| `OwnerPlayerUId`                             | StructProperty(Guid)| 所属玩家 UID                          |
| `SlotId`                                     | StructProperty      | 容器 ID + 槽位索引                    |
| `IsPlayer`                                   | BoolProperty        | 是否为玩家角色（区分玩家与帕鲁）      |
| `EquipWaza`                                  | ArrayProperty       | 已装备的主动技能列表                  |
| `PassiveSkillList`                           | ArrayProperty       | 被动技能列表                          |
| `WorkSuitability`                            | StructProperty      | 工作适应性等级 (生火/浇水/播种/手工等)|
| `Gender`                                     | EnumProperty        | 性别                                  |
| `IsBoss / IsRarePal`                         | BoolProperty        | Boss/稀有（闪光）标记                 |
| `GotStatusPointList`                         | StructProperty      | 魂强化点数 (HP/ATK/DEF/Craft 0-20)    |

---

## 5. 数据迁移：文件清单与操作流程

### 5.1 迁移所需核心文件

| 文件                | 必需性   | 用途                                                             |
| ------------------- | -------- | ---------------------------------------------------------------- |
| `Level.sav`         | **必需** | 世界数据：所有角色、建筑、据点、公会、物品容器、植被状态         |
| `LevelMeta.sav`     | 推荐     | 世界元数据：世界名称                                             |
| `WorldOption.sav`   | 可选     | 世界设置参数（如需保留原世界设置则迁移）                         |
| `Players/*.sav`     | **必需** | 所有玩家的个人存档（含科技点、地图解锁、物品栏）                 |
| `Players/*_dps.sav` | 推荐     | 玩家的帕鲁仓库数据                                               |

> **重要**
>
> **迁移前必须完全关闭游戏或停止服务器**。Palworld 在运行时会锁定存档文件，且自动存档会覆盖手动修改。所有操作应在服务器离线状态下进行。

### 5.2 场景一：本地主机迁移到专用服务器

这是最常见的迁移场景：将单人/联机主机的世界迁移到专用服务器 <sup>[[2]](#cite-2)</sup>。

1. **备份原始存档**：复制整个世界存档文件夹（`<WorldID>` 目录）到安全位置。
2. **复制存档文件**：将 `Level.sav`、`LevelMeta.sav` 和整个 `Players/` 文件夹复制到服务器的存档目录：
   - 目标路径: `PalServer\Pal\Saved\SaveGames\0\<ServerWorldID>\`
3. **修改服务器配置**：编辑 `PalServer\Pal\Saved\Config\WindowsServer\GameUserSettings.ini`，将 `DedicatedServerName` 设置为新的 `<ServerWorldID>` 目录名。
4. **启动服务器并创建新角色**：首次连接服务器时，游戏会为你分配一个新的 PlayerUID 并创建新角色存档。等待服务器自动保存（约 30 秒-1 分钟）。
5. **关闭服务器**：确认自动保存完成后，完全关闭服务器进程。
6. **使用 Fix Host Save 迁移 GUID**：在 PalworldSaveTools 中打开 `Level.sav`，使用 **Fix Host Save** 功能，选择旧角色（源）和新角色（目标），执行迁移。这将交换两个玩家的 UID 映射。
7. **重新启动服务器**：登录后你将拥有原来的角色、等级、帕鲁和物品。

### 5.3 Fix Host Save 的内部机制

`fix_host_save.py` 中的 `fix_save()` 函数执行了复杂的 **GUID 双向交换**操作 <sup>[[3]](#cite-3)</sup>。核心步骤如下：

**图 3：Fix Host Save GUID 交换流程**

```
┌─ Step 1: 解析三个文件 ─────────────────────────────────────────────┐
│ sav_to_json(Level.sav) → level_j                                    │
│ sav_to_json(old_player.sav) → old_j | sav_to_json(new_player.sav) → new_j
├─ Step 2: 交换 PlayerUId ────────────────────────────────────────────┤
│ old_j.PlayerUId → new_uid | new_j.PlayerUId → old_uid               │
│ 同时交换 IndividualId.PlayerUId                                     │
├─ Step 3: 更新 CharacterSaveParameterMap ────────────────────────────┤
│ 遍历所有角色条目，将 old_instance 的 PlayerUId → new_uid             │
│ 将 new_instance 的 PlayerUId → old_uid                              │
├─ Step 4: 更新公会数据 (GroupSaveDataMap) ───────────────────────────┤
│ 交换 admin_player_uid, player_uid, individual_character_handle_ids  │
│ 确保公会成员和会长的归属正确                                        │
├─ Step 5: 深度遍历交换所有者引用 ────────────────────────────────────┤
│ deep_swap(): 递归遍历整个 Level JSON，                               │
│ 交换 OwnerPlayerUId / build_player_uid / private_lock_player_uid    │
├─ Step 6: 重命名文件 + 重新压缩写入 ─────────────────────────────────┤
│ old.sav → new.sav | new.sav → old.sav | json_to_sav() 重新压缩所有文件│
└─────────────────────────────────────────────────────────────────────┘
```

核心代码的 `deep_swap()` 函数递归遍历整个 JSON 树，交换所有与 UID 相关的字段：

```python
def deep_swap(data):
    if isinstance(data, dict):
        # 检查所有可能包含玩家 UID 的字段
        for k in ('OwnerPlayerUId', 'owner_player_uid',
                 'build_player_uid', 'private_lock_player_uid'):
            v = data.get(k)
            if isinstance(v, dict) and v.get('value') == old_uid:
                v['value'] = new_uid          # 旧 → 新
            elif isinstance(v, dict) and v.get('value') == new_uid:
                v['value'] = old_uid          # 新 → 旧
            elif v == old_uid:
                data[k] = new_uid
            elif v == new_uid:
                data[k] = old_uid
        # 递归处理所有子节点
        for x in data.values():
            deep_swap(x)
    elif isinstance(data, list):
        for i in data:
            deep_swap(i)
```

### 5.4 场景二：主机更换（Host Swap）

当需要更换联机主机时，需要将旧主机的 `0001.sav` 与新主机的存档进行交换。这需要**两次** Fix Host Save 操作：

1. **第一次交换**：选择旧主机（0001.sav）为源，新主机（如 1234.sav）为目标，执行 Fix Host Save。结果：新主机获得 0001.sav 的进度，旧主机进度存入 1234.sav。
2. **新主机启动世界**：用新主机开服，确认角色、物品、帕鲁正确。
3. **旧主机加入**：旧主机加入新世界，可能被分配新的 UID（如 3456.sav）。创建临时角色，升至 2 级。
4. **第二次交换**：选择旧主机原始进度（1234.sav）为源，临时角色（3456.sav）为目标，执行 Fix Host Save。恢复旧主机的原始进度。

> **前提条件**
>
> Fix Host Save 要求双方角色等级均 **≥ 2 级**。新创建的角色必须先升到 2 级才能执行迁移。这是因为等级 1 的角色可能未完全初始化所有数据字段。

### 5.5 场景三：跨世界角色迁移（Character Transfer）

PalworldSaveTools 的 `character_transfer.py` 提供了更精细的角色迁移功能，可以在不同世界/服务器之间迁移单个或所有玩家，保留：

- **角色数据**：等级、属性、科技点
- **物品栏**：背包物品、装备、关键道具
- **帕鲁**：队伍帕鲁、帕鲁盒帕鲁
- **公会成员资格**：保留原公会归属
- **动态物品**：地面掉落物
- **时间戳**：最后在线时间等

### 5.6 迁移注意事项

| 问题                  | 原因                                                    | 解决方案                                                                                        |
| --------------------- | ------------------------------------------------------- | ----------------------------------------------------------------------------------------------- |
| 迁移后世界 ID 不匹配  | 服务器目录名与 `DedicatedServerName` 不一致             | 修改 `GameUserSettings.ini` 中的 `DedicatedServerName`，或删除 `WorldOption.sav` 让服务器重新生成 |
| 迁移后帕鲁丢失        | `_dps.sav` 文件未迁移或容器 ID 不匹配                   | 确保迁移 `Players/*_dps.sav` 文件；Fix Host Save 会自动更新 ContainerId                          |
| 迁移后建筑归属错误    | `build_player_uid` 未正确交换                           | `deep_swap()` 会递归处理，但建议迁移后验证建筑所有权                                             |
| `struct.error` 解析失败 | 存档格式版本过旧                                      | 先在游戏中加载一次存档（触发自动格式升级），再使用工具解析 <sup>[[1]](#cite-1)</sup>             |
| 迁移后公会数据丢失    | `GroupSaveDataMap` 中的 UID 未更新                      | 确保使用了 Fix Host Save 而非简单文件复制                                                        |

---

## Sources

1. <a id="cite-1"></a>**PalworldSaveTools, AGENTS.md** — 项目架构文档与关键约束。描述三层管道架构、压缩格式区分、GUI 与 CLI 解码差异、双存档位置设计。
   - 基于本地源码: `PalworldSaveTools-main/AGENTS.md`
2. <a id="cite-2"></a>**PalworldSaveTools, README.md** — 存档文件位置指南与主机→服务器迁移教程。包含本地和服务器存档路径、Host Swap 详细步骤。
   - 基于本地源码: `PalworldSaveTools-main/README.md`
3. <a id="cite-3"></a>**PalworldSaveTools, fix_host_save.py** — GUID 迁移核心实现。包含 `fix_save()`、`deep_swap()`、`copy_dps_file()` 等关键函数的完整代码。
   - 基于本地源码: `PalworldSaveTools-main/src/palworld_toolsets/fix_host_save.py`
