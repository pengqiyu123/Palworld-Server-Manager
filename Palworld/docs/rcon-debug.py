# 调试版：打印 RCON auth 响应的原始字节，判断是协议误判还是真拒
import socket, struct

HOST, PORT, PASSWORD = "127.0.0.1", 25575, "otGwh1JjCiEHx2Hx"

def make_packet(pid, ptype, body):
    body_bytes = body.encode("utf-8") + b"\x00"
    size = 4 + 4 + len(body_bytes)
    return struct.pack("<iii", size, pid, ptype) + body_bytes

def recv_exact(sock, n):
    buf = b""
    while len(buf) < n:
        chunk = sock.recv(n - len(buf))
        if not chunk: return None
        buf += chunk
    return buf

s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
s.settimeout(5)
s.connect((HOST, PORT))
print("connected")

# 发 auth
auth_pkt = make_packet(1, 3, PASSWORD)
print("send auth packet hex:", auth_pkt.hex())

# 读前 3 个返回包，全部 dump
for i in range(3):
    try:
        header = recv_exact(s, 4)
        if not header:
            print("pkt%d: no header" % i); break
        size = struct.unpack("<i", header)[0]
        print("pkt%d: size=%d" % (i, size))
        data = recv_exact(s, size)
        if not data:
            print("pkt%d: no body" % i); break
        pid = struct.unpack("<i", data[0:4])[0]
        ptype = struct.unpack("<i", data[4:8])[0]
        body_raw = data[8:]
        print("  pid=%d ptype=%d body_hex=%s body_text=%r" % (pid, ptype, body_raw.hex(), body_raw))
    except socket.timeout:
        print("pkt%d: timeout" % i); break

s.close()
