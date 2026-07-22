# 原始字节 dump：发 ShowPlayers，看服务器到底回不回、回什么
import socket, struct

HOST, PORT, PASSWORD = "127.0.0.1", 25575, "otGwh1JjCiEHx2Hx"

def make_packet(pid, ptype, body):
    body_bytes = body.encode("utf-8")
    size = 10 + len(body_bytes)
    return struct.pack("<iii", size, pid, ptype) + body_bytes + b"\x00\x00"

def recv_exact(sock, n):
    buf = b""
    while len(buf) < n:
        chunk = sock.recv(n - len(buf))
        if not chunk: return None
        buf += chunk
    return buf

s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
s.settimeout(10)
s.connect((HOST, PORT))

# auth
s.sendall(make_packet(1, 3, PASSWORD))
# 读 auth 响应（读到 type=2）
for _ in range(4):
    h = recv_exact(s, 4)
    sz = struct.unpack("<i", h)[0]
    d = recv_exact(s, sz)
    pid, ptype = struct.unpack("<ii", d[0:8])
    print("auth pkt: pid=%d type=%d size=%d" % (pid, ptype, sz))
    if ptype == 2:
        print("auth ok" if pid != -1 else "auth FAIL")
        break

# 排空
s.settimeout(0.3)
try:
    while True:
        d = s.recv(4096)
        if not d: break
        print("drained %d bytes" % len(d))
except socket.timeout:
    print("drain: nothing")
s.settimeout(10)

# 发 ShowPlayers，原始 dump
print("\n== send ShowPlayers ==")
s.sendall(make_packet(2, 2, "ShowPlayers"))
try:
    raw = s.recv(4096)
    print("recv %d bytes hex:" % len(raw))
    print(raw.hex())
    print("as text:", repr(raw))
except socket.timeout:
    print("TIMEOUT: 服务器对 ShowPlayers 完全没回")
s.close()
