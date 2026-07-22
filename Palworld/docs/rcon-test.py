# 最小 Valve RCON 客户端：连本地帕鲁服 25575，auth 后查 Info / ShowPlayers
# 证明 app 能通过 RCON 拿到实时服务器状态（不依赖读控制台窗口）
import socket, struct, sys

HOST, PORT, PASSWORD = "127.0.0.1", 25575, "otGwh1JjCiEHx2Hx"

SERVERDATA_AUTH = 3
SERVERDATA_EXECCOMMAND = 2
SERVERDATA_AUTH_RESPONSE = 2
SERVERDATA_RESPONSE_VALUE = 0

def make_packet(pid, ptype, body):
    # Valve RCON: 包尾必须 2 个 null（body 串终止符 + 包终止符）
    body_bytes = body.encode("utf-8")
    size = 10 + len(body_bytes)  # id(4) + type(4) + body + null(1) + null(1)
    return struct.pack("<iii", size, pid, ptype) + body_bytes + b"\x00\x00"

def recv_exact(sock, n):
    buf = b""
    while len(buf) < n:
        chunk = sock.recv(n - len(buf))
        if not chunk:
            return None
        buf += chunk
    return buf

def read_packet(sock):
    header = recv_exact(sock, 4)
    if not header:
        return None
    size = struct.unpack("<i", header)[0]
    data = recv_exact(sock, size)
    if not data:
        return None
    pid = struct.unpack("<i", data[0:4])[0]
    ptype = struct.unpack("<i", data[4:8])[0]
    body = data[8:].rstrip(b"\x00").decode("utf-8", errors="replace")
    return (pid, ptype, body)

def auth(sock, password):
    sock.sendall(make_packet(1, SERVERDATA_AUTH, password))
    # 读到 AUTH_RESPONSE(type=2) 为止（可能先来一个空 type=0）
    for _ in range(4):
        pkt = read_packet(sock)
        if pkt is None:
            return False, "连接断开"
        pid, ptype, body = pkt
        if ptype == SERVERDATA_AUTH_RESPONSE:
            if pid == -1 or pid == 0xFFFFFFFF:
                return False, "密码错误(auth rejected)"
            return True, "auth OK, id=%d" % pid
    return False, "未收到 AUTH_RESPONSE"

def exec_cmd(sock, cmd):
    sock.sendall(make_packet(2, SERVERDATA_EXECCOMMAND, cmd))
    # 帕鲁 RCON 可能先回一个空包，读到非空 type=0 或读满 3 包为止
    for _ in range(3):
        pkt = read_packet(sock)
        if pkt is None:
            return "(无响应)"
        pid, ptype, body = pkt
        if ptype == SERVERDATA_RESPONSE_VALUE and body.strip():
            return body
    return "(响应为空——可能该指令无输出或需 base64 解码)"

def main():
    print("== 连接 %s:%d ==" % (HOST, PORT))
    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    s.settimeout(10)
    try:
        s.connect((HOST, PORT))
    except Exception as e:
        print("连接失败:", e); return
    ok, msg = auth(s, PASSWORD)
    print("AUTH:", ok, msg)
    if not ok:
        s.close(); return
    # auth 后排空残留包（帕鲁可能留个空 type=0 把流对齐搞乱）
    s.settimeout(0.3)
    try:
        while True:
            d = s.recv(4096)
            if not d:
                break
    except socket.timeout:
        pass
    s.settimeout(10)
    for cmd in ["Info", "ShowPlayers"]:
        print("\n== RCON %s ==\n%s" % (cmd, "-"*40))
        print(exec_cmd(s, cmd))
    s.close()

if __name__ == "__main__":
    main()
