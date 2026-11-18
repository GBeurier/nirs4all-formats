"""Generate CC0 NSAS DA/SPC stand-ins without any customer measurements."""

import struct
from pathlib import Path

ROOT = Path(__file__).parent


def generate() -> tuple[bytes, bytes]:
    da = bytearray()
    spc = bytearray(512)
    struct.pack_into(
        "<BBBBIddIBBB", spc, 0, 4, 0x4B, 0, 128, 1050, 400, 2498, 3, 3, 2, 0
    )
    for row, (sample_id, number) in enumerate(
        [("DEMO-A", 1), ("DEMO-A", 2), ("DEMO-B", 2)]
    ):
        header = bytearray(128)
        header[4:14] = sample_id.ljust(10).encode("ascii")
        for offset, value in [
            (0, 6500),
            (2, number),
            (14, 1050),
            (16, 400),
            (18, 2500),
            (20, 1),
            (22, 2),
            (52, 1),
            (54, 1026),
            (56, 1),
            (58, 1),
            (60, 2),
            (64, 29),
            (66, 2),
            (68, 2000),
            (70, 12),
            (72, row),
            (74, 30),
            (76, 32),
            (78, 2000),
        ]:
            struct.pack_into("<H", header, offset, value)
        struct.pack_into("<4f", header, 80, -0.25, 0.5, 5.75, 1)
        values = struct.pack(
            "<1050f", *(0.5 + row * 0.125 + i / 4096 for i in range(1050))
        )
        da.extend(header + values + b" " * 24)
        sub = bytearray(32)
        struct.pack_into("<BBHfffIIf", sub, 0, 0, 128, row, row, row + 1, 0, 0, 32, 0)
        spc.extend(sub + values)
    return bytes(da), bytes(spc)


def generate_variant(
    points: int, start: int, end: int, rows: int, count: int = 0
) -> bytes:
    """Generate invented signals in observed geometries, without vendor payloads."""
    data = bytearray()
    stride = ((128 + 4 * points + 127) // 128) * 128
    for row in range(rows):
        header = bytearray(128)
        header[4:14] = f"SYN-{row}".encode("ascii").ljust(10, b"\0")
        for offset, value in [
            (0, [0, 6500, 5000][row % 3]),
            (2, row + 1),
            (14, points),
            (16, start),
            (18, end),
            (20, 11 if points in (1700, 4200) else 1),
            (22, 2),
            (30, count),
            (52, 2),
            (54, 1025),
            (64, 18),
            (66, 11),
            (68, 2004),
            (70, 10),
            (72, row),
            (74, 30),
            (76, 32),
            (78, 2004),
        ]:
            struct.pack_into("<H", header, offset, value)
        struct.pack_into("<4f", header, 80, -0.25, 0.5, 5.75, 1)
        payload = struct.pack(
            f"<{points}f", *(row * 0.125 + i / 8192 for i in range(points))
        )
        data.extend(header + payload + b"\0" * (stride - 128 - len(payload)))
    return bytes(data)


def generate_cn() -> bytes:
    """Two invented targets exercise indexed slots; no fitted model is included."""
    data = bytearray(384)
    data[:10] = b"WATER".ljust(10, b" ")
    data[10:20] = b"Oil".ljust(10, b" ")
    for water, oil in [(1.25, 0.0), (0.0, 3.25), (2.5, 1.5)]:
        data.extend(struct.pack("<32f", water, oil, *([0.0] * 30)))
    return bytes(data)


if __name__ == "__main__":
    da, spc = generate()
    (ROOT / "synthetic.da").write_bytes(da)
    (ROOT / "synthetic.spc").write_bytes(spc)
    (ROOT / "references700.da").write_bytes(generate_variant(700, 1100, 2500, 3, 2))
    (ROOT / "references700.cn").write_bytes(generate_cn())
    (ROOT / "variant1700.da").write_bytes(generate_variant(1700, 800, 1650, 2))
    (ROOT / "variant4200.da").write_bytes(generate_variant(4200, 400, 2500, 2))
