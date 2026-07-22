# 一次性脚本：把 DefaultPalWorldSettings.ini 模板填充进空的 PalWorldSettings.ini
# 只改 3 个字段：ServerName / AdminPassword(随机) / RCONEnabled=True
import secrets, string, sys

TEMPLATE = r"E:\SteamLibrary\steamapps\common\PalServer\DefaultPalWorldSettings.ini"
TARGET   = r"E:\SteamLibrary\steamapps\common\PalServer\Pal\Saved\Config\WindowsServer\PalWorldSettings.ini"

with open(TEMPLATE, "r", encoding="utf-8") as f:
    lines = f.readlines()

# 去掉模板开头的 3 行注释，只保留 [/Script...] + OptionSettings
out = [ln for ln in lines if not ln.lstrip().startswith(";")]
content = "".join(out)

# 生成 16 位随机管理员密码（字母+数字，避开易混字符）
alphabet = "".join(c for c in (string.ascii_letters + string.digits) if c not in "IlO0")
admin_pw = "".join(secrets.choice(alphabet) for _ in range(16))

# 精确替换 3 个字段
content = content.replace('ServerName="Default Palworld Server"', 'ServerName="煜的帕鲁世界"')
content = content.replace('AdminPassword=""', f'AdminPassword="{admin_pw}"')
content = content.replace('RCONEnabled=False', 'RCONEnabled=True')

with open(TARGET, "w", encoding="utf-8") as f:
    f.write(content)

# 输出密码供主理人记录（唯一一次明文）
print("WROTE:", TARGET)
print("ADMIN_PASSWORD=" + admin_pw)
print("SIZE_BYTES=" + str(len(content.encode("utf-8"))))
