"""Log every inbound MIDI message from the RE-202 as JSONL.

Usage:
    py -3 capture.py --list-ports
    py -3 capture.py --input "RE-202" --out captures/baseline.jsonl

The output is one JSON object per line:
    {"t": 1234.567, "type": "sysex", "hex": "F0 41 10 ... F7", "decoded": {...}}

Decoded field uses the Roland RE-202 framing if it parses; otherwise just
records the type.

Not shipped — throwaway exploration code.
"""
from __future__ import annotations

import argparse
import datetime as dt
import json
import pathlib
import signal
import sys
import time

import mido


SYSEX_START = 0xF0
SYSEX_END = 0xF7
ROLAND_ID = 0x41
RE202_MODEL_ID = bytes([0x00, 0x00, 0x00, 0x00, 0x18])


def hexs(data: bytes) -> str:
    return " ".join(f"{b:02X}" for b in data)


def roland_checksum(addr_and_data: bytes) -> int:
    s = sum(addr_and_data) % 128
    return (128 - s) % 128


def try_decode_re202(raw: bytes) -> dict | None:
    """Best-effort decode of an RE-202 SysEx frame. Returns None if shape mismatches."""
    if len(raw) < 15:
        return None
    if raw[0] != SYSEX_START or raw[-1] != SYSEX_END:
        return None
    if raw[1] != ROLAND_ID:
        return None
    device_id = raw[2]
    if raw[3:8] != RE202_MODEL_ID:
        return None
    cmd = raw[8]
    addr = raw[9:13]
    payload_end = len(raw) - 2
    data = raw[13:payload_end]
    chk = raw[payload_end]
    expected = roland_checksum(addr + data)
    return {
        "device_id": f"0x{device_id:02X}",
        "command": f"0x{cmd:02X}",
        "command_name": {0x11: "RQ1", 0x12: "DT1"}.get(cmd, f"0x{cmd:02X}"),
        "address": hexs(addr),
        "data_len": len(data),
        "data": hexs(data) if len(data) <= 64 else hexs(data[:64]) + " ... (truncated)",
        "checksum_ok": chk == expected,
        "checksum_expected": f"0x{expected:02X}",
        "checksum_got": f"0x{chk:02X}",
    }


def list_ports() -> None:
    print("Input ports:")
    for name in mido.get_input_names():
        print(f"  - {name}")
    print()
    print("Output ports:")
    for name in mido.get_output_names():
        print(f"  - {name}")


def pick_port(names: list[str], needle: str) -> str:
    matches = [n for n in names if needle.lower() in n.lower()]
    if not matches:
        sys.exit(f"No input port matches {needle!r}. Available: {names}")
    if len(matches) > 1:
        print(f"Multiple matches for {needle!r}, picking first: {matches[0]}", file=sys.stderr)
    return matches[0]


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--list-ports", action="store_true")
    ap.add_argument("--input", help="substring of the MIDI input port name (e.g. 'RE-202')")
    ap.add_argument("--out", type=pathlib.Path, help="output JSONL path")
    ap.add_argument("--echo", action="store_true", help="also print decoded frames to stdout")
    args = ap.parse_args()

    if args.list_ports:
        list_ports()
        return
    if not args.input or not args.out:
        ap.error("--input and --out are required unless --list-ports is used")

    port_name = pick_port(mido.get_input_names(), args.input)
    args.out.parent.mkdir(parents=True, exist_ok=True)

    print(f"Capturing from {port_name!r} -> {args.out}", file=sys.stderr)
    print("Ctrl-C to stop.", file=sys.stderr)

    start = time.monotonic()
    stop_requested = False

    def handle_sigint(_signum, _frame):
        nonlocal stop_requested
        stop_requested = True

    signal.signal(signal.SIGINT, handle_sigint)

    with mido.open_input(port_name) as inp, args.out.open("w", encoding="utf-8") as f:
        # Write a header row with capture metadata.
        meta = {
            "_meta": True,
            "started": dt.datetime.now().isoformat(),
            "port": port_name,
        }
        f.write(json.dumps(meta) + "\n")
        f.flush()

        while not stop_requested:
            msg = inp.poll()
            if msg is None:
                time.sleep(0.001)
                continue
            t = round(time.monotonic() - start, 6)
            entry: dict = {"t": t, "type": msg.type}
            if msg.type == "sysex":
                raw = bytes([SYSEX_START]) + bytes(msg.data) + bytes([SYSEX_END])
                entry["hex"] = hexs(raw)
                decoded = try_decode_re202(raw)
                if decoded is not None:
                    entry["decoded"] = decoded
            else:
                entry["repr"] = str(msg)
            f.write(json.dumps(entry) + "\n")
            f.flush()
            if args.echo:
                print(entry)

    print("Capture stopped.", file=sys.stderr)


if __name__ == "__main__":
    main()
