"""Drive the RE-202 through several memories via PC# and read `7F 00 00 00`
after each. If the byte changes with the active memory, it tracks the slot.

Usage:
    py -3 probe_current_memory_sweep.py --port Focusrite

Not shipped — exploration code.
"""
from __future__ import annotations

import argparse
import sys
import time

import mido

SYSEX_START = 0xF0
SYSEX_END = 0xF7
ROLAND_ID = 0x41
RE202_MODEL_ID = bytes([0x00, 0x00, 0x00, 0x00, 0x18])
CMD_RQ1 = 0x11


def roland_checksum(addr_and_data: bytes) -> int:
    s = sum(addr_and_data) % 128
    return (128 - s) % 128


def build_rq1(device_id, address, size):
    body = address + size
    chk = roland_checksum(body)
    return (
        bytes([SYSEX_START, ROLAND_ID, device_id])
        + RE202_MODEL_ID
        + bytes([CMD_RQ1])
        + body
        + bytes([chk, SYSEX_END])
    )


def hexs(data):
    return " ".join(f"{b:02X}" for b in data)


def pick(names, needle):
    matches = [n for n in names if needle.lower() in n.lower()]
    if not matches:
        sys.exit(f"No port matches {needle!r}. Available: {names}")
    return matches[0]


def read_addr(out, inp, device_id, address, size, timeout_ms=800):
    size_bytes = bytes(
        [(size >> 21) & 0x7F, (size >> 14) & 0x7F, (size >> 7) & 0x7F, size & 0x7F]
    )
    frame = build_rq1(device_id, address, size_bytes)
    while inp.poll() is not None:
        pass
    out.send(mido.Message("sysex", data=list(frame[1:-1])))
    deadline = time.monotonic() + timeout_ms / 1000.0
    while time.monotonic() < deadline:
        msg = inp.poll()
        if msg is None:
            time.sleep(0.001)
            continue
        if msg.type != "sysex":
            continue
        raw = bytes(msg.data)
        if len(raw) < 12 or raw[0] != ROLAND_ID:
            continue
        if raw[2:7] != RE202_MODEL_ID:
            continue
        if raw[7] != 0x12:
            continue
        if raw[8:12] != address:
            continue
        return raw[12:-1]
    return None


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--port", default="Focusrite")
    ap.add_argument("--input")
    ap.add_argument("--output")
    ap.add_argument("--device-id", type=lambda s: int(s, 0), default=0x10)
    args = ap.parse_args()

    in_needle = args.input or args.port
    out_needle = args.output or args.port

    in_name = pick(mido.get_input_names(), in_needle)
    out_name = pick(mido.get_output_names(), out_needle)
    print(f"# input:  {in_name}")
    print(f"# output: {out_name}")

    probe_addrs = [
        bytes([0x7F, 0x00, 0x00, 0x00]),
        # Edit-buffer "Mode" byte (offset 0x01 of memory block). Sanity check
        # that PC sends are actually switching memory.
        bytes([0x20, 0x00, 0x00, 0x01]),
    ]

    pc_targets = [0, 1, 2, 5, 50, 127, 1]  # MANUAL, M1, M2, M5, M50, M127, back to M1

    with mido.open_input(in_name) as inp, mido.open_output(out_name) as out:
        rows = []

        # Initial read with whatever's currently active.
        readings = [read_addr(out, inp, args.device_id, a, 1) for a in probe_addrs]
        rows.append(("initial (unknown)", readings))

        for pc in pc_targets:
            # Send PC on channel 1 (status 0xC0).
            out.send(mido.Message("program_change", channel=0, program=pc))
            # Give the device a moment to swap memory.
            time.sleep(0.15)
            readings = [read_addr(out, inp, args.device_id, a, 1) for a in probe_addrs]
            label = "MANUAL" if pc == 0 else f"MEMORY {pc}"
            rows.append((f"after PC#{pc} ({label})", readings))

        print()
        print(f"{'state':36s}  " + "  ".join(hexs(a) for a in probe_addrs))
        for label, readings in rows:
            shown = "  ".join(
                hexs(r) if r is not None else "---" for r in readings
            )
            print(f"{label:36s}  {shown}")

        # Verdict
        col_vals = [bytes(r[0]) if r[0] is not None else None for _, r in rows]
        non_null = [v for v in col_vals if v is not None]
        if non_null:
            uniq = set(non_null)
            print()
            if len(uniq) > 1:
                print(f"VERDICT: {hexs(probe_addrs[0])} CHANGES across memories ({len(uniq)} distinct values). It tracks the active slot.")
            else:
                print(f"VERDICT: {hexs(probe_addrs[0])} returned {hexs(non_null[0])} in EVERY state. It does NOT track the active slot.")


if __name__ == "__main__":
    main()
