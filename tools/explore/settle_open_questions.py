"""Settle the four small open questions in `docs/sysex-notes.md`.

1. What is at 7F 00 00 00? — probe sizes 1, 2, 4, 8.
2. PC# → memory mapping. — send PC 0, 1, 2; for each, read the edit buffer
   and compare against the corresponding slot.
3. Broadcast device id (0x7F) — does the device respond?
4. Time Mode precedence — change System Time Mode; observe what the edit
   buffer's Time Mode does. Then change per-memory Time Mode; observe.
"""
from __future__ import annotations

import sys
import time

import mido

SYSEX_START = 0xF0
SYSEX_END = 0xF7
ROLAND_ID = 0x41
RE202_MODEL_ID = bytes([0x00, 0x00, 0x00, 0x00, 0x18])
CMD_RQ1 = 0x11
CMD_DT1 = 0x12


def chk(data: list[int] | bytes) -> int:
    return (128 - (sum(data) % 128)) % 128


def hexs(b: bytes | list[int]) -> str:
    return " ".join(f"{x:02X}" for x in b)


def build(dev: int, command: int, address: bytes, payload: bytes) -> bytes:
    body = address + payload
    return (
        bytes([SYSEX_START, ROLAND_ID, dev])
        + RE202_MODEL_ID
        + bytes([command])
        + body
        + bytes([chk(body), SYSEX_END])
    )


def rq1(dev: int, address: bytes, size_bytes: bytes) -> bytes:
    return build(dev, CMD_RQ1, address, size_bytes)


def dt1(dev: int, address: bytes, data: bytes) -> bytes:
    return build(dev, CMD_DT1, address, data)


def pick(names: list[str], needle: str) -> str:
    return next(n for n in names if needle.lower() in n.lower())


def listen_sysex(inp, ms: int) -> list[bytes]:
    deadline = time.monotonic() + ms / 1000.0
    out: list[bytes] = []
    while time.monotonic() < deadline:
        msg = inp.poll()
        if msg is None:
            time.sleep(0.002)
            continue
        if msg.type == "sysex":
            raw = bytes([SYSEX_START]) + bytes(msg.data) + bytes([SYSEX_END])
            # Skip our own echoes (RQ1/DT1 to/from same device).
            out.append(raw)
    # Filter responses: only DT1 frames (cmd byte at index 8 == 0x12).
    return [r for r in out if len(r) >= 10 and r[8] == CMD_DT1]


def drain(inp) -> None:
    while inp.poll() is not None:
        pass


def main() -> None:
    in_name = pick(mido.get_input_names(), "Focusrite")
    out_name = pick(mido.get_output_names(), "Focusrite")
    with mido.open_input(in_name) as inp, mido.open_output(out_name) as out:

        # ============================================================
        print("=" * 60)
        print("Q1. Probe 7F 00 00 00 with sizes 1, 2, 4, 8")
        print("=" * 60)
        for size in (1, 2, 4, 8):
            drain(inp)
            frame = rq1(0x10, bytes([0x7F, 0, 0, 0]), bytes([0, 0, 0, size]))
            out.send(mido.Message("sysex", data=list(frame[1:-1])))
            replies = listen_sysex(inp, 600)
            if replies:
                data_section = [hexs(r[13:-2]) for r in replies]
                print(f"  size={size:>2}: replies={data_section}")
            else:
                print(f"  size={size:>2}: (no reply)")

        # ============================================================
        print()
        print("=" * 60)
        print("Q2. PC# -> memory mapping")
        print("=" * 60)
        # Read MEMORY MANUAL and MEMORY 1 reference snapshots
        def read_memory(addr: bytes) -> bytes | None:
            drain(inp)
            f = rq1(0x10, addr, bytes([0, 0, 0, 0x21]))
            out.send(mido.Message("sysex", data=list(f[1:-1])))
            r = listen_sysex(inp, 800)
            if not r:
                return None
            # 33 data bytes between addr (4) and checksum
            return r[0][13:-2]

        mem_manual = read_memory(bytes([0x20, 0x10, 0, 0]))
        mem_1 = read_memory(bytes([0x20, 0x20, 0, 0]))
        mem_2 = read_memory(bytes([0x20, 0x30, 0, 0]))

        for pc in (0, 1, 2):
            drain(inp)
            out.send(mido.Message("program_change", channel=0, program=pc))
            time.sleep(0.15)
            edit = read_memory(bytes([0x20, 0x00, 0, 0]))
            if edit is None:
                print(f"  PC#{pc}: no edit-buffer response")
                continue
            label = "unknown"
            if mem_manual is not None and edit == mem_manual:
                label = "MANUAL"
            elif mem_1 is not None and edit == mem_1:
                label = "MEMORY 1"
            elif mem_2 is not None and edit == mem_2:
                label = "MEMORY 2"
            print(f"  PC#{pc} maps to edit buffer = {label}")

        # ============================================================
        print()
        print("=" * 60)
        print("Q3. Broadcast device id 0x7F")
        print("=" * 60)
        drain(inp)
        frame = rq1(0x7F, bytes([0x10, 0, 0, 0]), bytes([0, 0, 0, 1]))
        out.send(mido.Message("sysex", data=list(frame[1:-1])))
        replies = listen_sysex(inp, 800)
        if replies:
            print(f"  broadcast RQ1 returned {len(replies)} reply:")
            for r in replies:
                print(f"    {hexs(r)}")
        else:
            print("  broadcast RQ1: (no reply)")

        # ============================================================
        print()
        print("=" * 60)
        print("Q4. Time Mode precedence: System (10 00 00 06) vs per-memory (offset 0x20)")
        print("=" * 60)

        def read_byte(addr: bytes) -> int | None:
            drain(inp)
            f = rq1(0x10, addr, bytes([0, 0, 0, 1]))
            out.send(mido.Message("sysex", data=list(f[1:-1])))
            r = listen_sysex(inp, 600)
            if not r:
                return None
            return r[0][13]  # first data byte

        sys_tm = read_byte(bytes([0x10, 0, 0, 0x06]))
        # Per-memory Time Mode is at memory_base + 0x20 — for the edit buffer that's 20 00 00 20.
        mem_tm = read_byte(bytes([0x20, 0, 0, 0x20]))
        print(f"  baseline: System Time Mode={sys_tm}, Edit-buffer Time Mode={mem_tm}")

        print("  writing System Time Mode = 1 (LONG)")
        body = bytes([0x10, 0, 0, 0x06, 1])
        f = build(0x10, CMD_DT1, body[:4], body[4:])
        out.send(mido.Message("sysex", data=list(f[1:-1])))
        time.sleep(0.1)
        sys_tm2 = read_byte(bytes([0x10, 0, 0, 0x06]))
        mem_tm2 = read_byte(bytes([0x20, 0, 0, 0x20]))
        print(f"  after: System Time Mode={sys_tm2}, Edit-buffer Time Mode={mem_tm2}")

        print("  writing per-memory Time Mode = 0 (NORMAL)")
        body = bytes([0x20, 0, 0, 0x20, 0])
        f = build(0x10, CMD_DT1, body[:4], body[4:])
        out.send(mido.Message("sysex", data=list(f[1:-1])))
        time.sleep(0.1)
        sys_tm3 = read_byte(bytes([0x10, 0, 0, 0x06]))
        mem_tm3 = read_byte(bytes([0x20, 0, 0, 0x20]))
        print(f"  after: System Time Mode={sys_tm3}, Edit-buffer Time Mode={mem_tm3}")

        # Restore baseline
        print("  restoring System Time Mode and Edit-buffer Time Mode to baselines")
        if sys_tm is not None:
            body = bytes([0x10, 0, 0, 0x06, sys_tm])
            f = build(0x10, CMD_DT1, body[:4], body[4:])
            out.send(mido.Message("sysex", data=list(f[1:-1])))
            time.sleep(0.05)
        if mem_tm is not None:
            body = bytes([0x20, 0, 0, 0x20, mem_tm])
            f = build(0x10, CMD_DT1, body[:4], body[4:])
            out.send(mido.Message("sysex", data=list(f[1:-1])))


if __name__ == "__main__":
    try:
        main()
    except Exception as e:
        print(f"ERROR: {e}", file=sys.stderr)
        raise
