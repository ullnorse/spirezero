"""Prints a QR code of a short text (the LAN URL) in the terminal, for the phone's camera.
Stdlib only: byte mode, error correction level L, versions 1-6 (up to 134 bytes), mask 0.
Usage: qr.py <text>"""
import sys

# version -> (data codewords per block, blocks, EC codewords per block), level L
TABLE = {1: (19, 1, 7), 2: (34, 1, 10), 3: (55, 1, 15), 4: (80, 1, 20), 5: (108, 1, 26), 6: (68, 2, 18)}
ALIGN = {1: [], 2: [6, 18], 3: [6, 22], 4: [6, 26], 5: [6, 30], 6: [6, 34]}

EXP = [0] * 512
LOG = [0] * 256
x = 1
for i in range(255):
    EXP[i] = x
    LOG[x] = i
    x <<= 1
    if x & 0x100:
        x ^= 0x11D
for i in range(255, 512):
    EXP[i] = EXP[i - 255]


def mul(a, b):
    return 0 if a == 0 or b == 0 else EXP[LOG[a] + LOG[b]]


def ec_codewords(data, n):
    gen = [1]
    for i in range(n):
        gen = [a ^ mul(b, EXP[i]) for a, b in zip(gen + [0], [0] + gen)]
    rem = list(data) + [0] * n
    for i in range(len(data)):
        f = rem[i]
        if f:
            for j, g in enumerate(gen):
                rem[i + j] ^= mul(g, f)
    return rem[len(data):]


def codewords(text):
    raw = text.encode()
    for version, (per, blocks, ec) in TABLE.items():
        if len(raw) + 2 <= per * blocks:
            break
    else:
        raise SystemExit("text too long for a small QR code")
    bits = "0100" + f"{len(raw):08b}" + "".join(f"{b:08b}" for b in raw)
    cap = per * blocks * 8
    bits += "0" * min(4, cap - len(bits))
    bits += "0" * (-len(bits) % 8)
    data = [int(bits[i:i + 8], 2) for i in range(0, len(bits), 8)]
    filled = len(data)
    while len(data) < per * blocks:
        data.append((0xEC, 0x11)[(len(data) - filled) % 2])
    split = [data[i * per:(i + 1) * per] for i in range(blocks)]
    ecs = [ec_codewords(b, ec) for b in split]
    out = [b[i] for i in range(per) for b in split] + [e[i] for i in range(ec) for e in ecs]
    return version, out


def matrix(text):
    version, data = codewords(text)
    size = 17 + 4 * version
    dark = [[False] * size for _ in range(size)]
    fixed = [[False] * size for _ in range(size)]

    def put(x, y, v):
        dark[y][x] = v
        fixed[y][x] = True

    for i in range(size):
        put(6, i, i % 2 == 0)
        put(i, 6, i % 2 == 0)
    for cx, cy in ((3, 3), (size - 4, 3), (3, size - 4)):
        for dy in range(-4, 5):
            for dx in range(-4, 5):
                if 0 <= cx + dx < size and 0 <= cy + dy < size:
                    put(cx + dx, cy + dy, max(abs(dx), abs(dy)) not in (2, 4))
    centers = ALIGN[version]
    for cy in centers:
        for cx in centers:
            if (cx, cy) in ((6, 6), (6, size - 7), (size - 7, 6)):
                continue
            for dy in range(-2, 3):
                for dx in range(-2, 3):
                    put(cx + dx, cy + dy, max(abs(dx), abs(dy)) != 1)

    mask = 0
    fmt = (1 << 3) | mask  # level L is 01
    rem = fmt
    for _ in range(10):
        rem = (rem << 1) ^ ((rem >> 9) * 0x537)
    fbits = ((fmt << 10) | rem) ^ 0x5412
    bit = lambda i: (fbits >> i) & 1 == 1
    for i in range(6):
        put(8, i, bit(i))
    put(8, 7, bit(6))
    put(8, 8, bit(7))
    put(7, 8, bit(8))
    for i in range(9, 15):
        put(14 - i, 8, bit(i))
    for i in range(8):
        put(size - 1 - i, 8, bit(i))
    for i in range(8, 15):
        put(8, size - 15 + i, bit(i))
    put(8, size - 8, True)

    i = 0
    right = size - 1
    while right >= 1:
        if right == 6:
            right = 5
        for vert in range(size):
            for j in range(2):
                x = right - j
                y = size - 1 - vert if (right + 1) & 2 == 0 else vert
                if not fixed[y][x]:
                    v = i < len(data) * 8 and (data[i >> 3] >> (7 - (i & 7))) & 1 == 1
                    dark[y][x] = v ^ ((x + y) % 2 == 0)
                    i += 1
        right -= 2
    return dark


def show(text):
    m = matrix(text)
    quiet = 4
    n = len(m) + 2 * quiet
    light = lambda x, y: not (quiet <= x < n - quiet and quiet <= y < n - quiet and m[y - quiet][x - quiet])
    # Light modules are drawn (the terminal's text is light on dark), dark ones left blank.
    for y in range(0, n, 2):
        line = ""
        for x in range(n):
            top = light(x, y)
            bottom = light(x, y + 1) if y + 1 < n else False
            line += "█" if top and bottom else "▀" if top else "▄" if bottom else " "
        print(line)


if __name__ == "__main__":
    show(sys.argv[1])
