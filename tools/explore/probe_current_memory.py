"""One-shot RQ1 reader for hardware probes. Prints the data bytes of the DT1
reply for one or more addresses, ignoring echoes and unrelated traffic.

Used to test whether `7F 00 00 00` (or another candidate address) tracks the
currently active memory slot: call this repeatedly while changing memory on
the device, and compare the bytes.

Usage:
    py -3 probe_current_memory.py --port Focusrite
    py -3 probe_current_memory.py --port Focusrite --address 7F000000 --size 1

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


def build_rq1(device_id: int, address: bytes, size: bytes) -> bytes:
    body = address + size
    chk = roland_checksum(body)
    return (
        bytes([SYSEX_START, ROLAND_ID, device_id])
        + RE202_MODEL_ID
        + bytes([CMD_RQ1])
        + body
        + bytes([chk, SYSEX_END])
    )


def hexs(data: bytes) -> str:
    return " ".join(f"{b:02X}" for b in data)


def pick(names: list[str], needle: str) -> str:
    matches = [n for n in names if needle.lower() in n.lower()]
    if not matches:
        sys.exit(f"No port matches {needle!r}. Available: {names}")
    if len(matches) > 1:
        print(f"# multiple ports match {needle!r}; using first: {matches[0]}", file=sys.stderr)
    return matches[0]


def read_address(out, inp, device_id, address, size, timeout_ms=600):
    size_bytes = bytes(
        [
            (size >> 21) & 0x7F,
            (size >> 14) & 0x7F,
            (size >> 7) & 0x7F,
            size & 0x7F,
        ]
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
        reply_addr = raw[8:12]
        if reply_addr != address:
            continue
        return raw[12:-1]
    return None


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--port", default="Focusrite")
    ap.add_argument("--input")
    ap.add_argument("--output")
    ap.add_argument("--device-id", type=lambda s: int(s, 0), default=0x10)
    ap.add_argument("--address", default="7F000000", help="4-byte address as 8 hex chars")
    ap.add_argument("--size", type=int, default=1)
    args = ap.parse_args()

    in_needle = args.input or args.port
    out_needle = args.output or args.port

    in_name = pick(mido.get_input_names(), in_needle)
    out_name = pick(mido.get_output_names(), out_needle)
    addr_bytes = bytes.fromhex(args.address)
    if len(addr_bytes) != 4:
        sys.exit(f"--address must be 4 bytes; got {len(addr_bytes)}")

    with mido.open_input(in_name) as inp, mido.open_output(out_name) as out:
        data = read_address(out, inp, args.device_id, addr_bytes, args.size)

    if data is None:
        print(f"{hexs(addr_bytes)} size {args.size}: (no reply within timeout)")
        sys.exit(1)
    print(f"{hexs(addr_bytes)} size {args.size}: {hexs(data)}")


if __name__ == "__main__":
    main()
