"""Generate CC0 NSAS DA/SPC stand-ins without any customer measurements."""

import struct
from pathlib import Path

ROOT = Path(__file__).parent


def generate() -> tuple[bytes, bytes]:
    da = bytearray()
    spc = bytearray(512)
    struct.pack_into("<BBBBIddIBBB", spc, 0, 4, 0x4B, 0, 128, 1050, 400, 2498, 3, 3, 2, 0)
    for row, (sample_id, number) in enumerate([("DEMO-A", 1), ("DEMO-A", 2), ("DEMO-B", 2)]):
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
        values = struct.pack("<1050f", *(0.5 + row * 0.125 + i / 4096 for i in range(1050)))
        da.extend(header + values + b" " * 24)
        sub = bytearray(32)
        struct.pack_into("<BBHfffIIf", sub, 0, 0, 128, row, row, row + 1, 0, 0, 32, 0)
        spc.extend(sub + values)
    return bytes(da), bytes(spc)


if __name__ == "__main__":
    da, spc = generate()
    (ROOT / "synthetic.da").write_bytes(da)
    (ROOT / "synthetic.spc").write_bytes(spc)
