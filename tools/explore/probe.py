"""Send a hypothesized RE-202 RQ1 or DT1 and print whatever the device returns.

Usage:
    py -3 probe.py rq1 --input "RE-202" --output "RE-202" --address 10000000 --size 00000010
    py -3 probe.py dt1 --input "RE-202" --output "RE-202" --address 10000000 --data 00
    py -3 probe.py sweep --input "RE-202" --output "RE-202" --base 10000000 --count 32

Addresses, sizes, and data are hex without spaces. Device id defaults to 0x10.

Not shipped — throwaway exploration code.
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
CMD_DT1 = 0x12


def roland_checksum(addr_and_data: bytes) -> int:
    s = sum(addr_and_data) % 128
    return (128 - s) % 128


def hexs(data: bytes) -> str:
    return " ".join(f"{b:02X}" for b in data)


def build(device_id: int, command: int, address: bytes, data: bytes) -> bytes:
    body = address + data
    chk = roland_checksum(body)
    return bytes(
        [SYSEX_START, ROLAND_ID, device_id]
    ) + RE202_MODEL_ID + bytes([command]) + body + bytes([chk, SYSEX_END])


def pick(names: list[str], needle: str) -> str:
    matches = [n for n in names if needle.lower() in n.lower()]
    if not matches:
        sys.exit(f"No port matches {needle!r}. Available: {names}")
    return matches[0]


def parse_hex(s: str, want_len: int | None = None) -> bytes:
    raw = bytes.fromhex(s)
    if want_len is not None and len(raw) != want_len:
        sys.exit(f"expected {want_len} hex bytes, got {len(raw)}")
    return raw


def listen_briefly(inp, ms: int = 500) -> list[mido.Message]:
    deadline = time.monotonic() + ms / 1000.0
    received: list[mido.Message] = []
    while time.monotonic() < deadline:
        msg = inp.poll()
        if msg is None:
            time.sleep(0.001)
            continue
        received.append(msg)
    return received


def cmd_rq1(args) -> None:
    addr = parse_hex(args.address, want_len=4)
    size = parse_hex(args.size, want_len=4)
    frame = build(args.device_id, CMD_RQ1, addr, size)
    print(f"-> {hexs(frame)}", file=sys.stderr)
    in_name = pick(mido.get_input_names(), args.input)
    out_name = pick(mido.get_output_names(), args.output)
    with mido.open_input(in_name) as inp, mido.open_output(out_name) as out:
        out.send(mido.Message("sysex", data=list(frame[1:-1])))
        for msg in listen_briefly(inp, args.timeout_ms):
            if msg.type == "sysex":
                raw = bytes([SYSEX_START]) + bytes(msg.data) + bytes([SYSEX_END])
                print(f"<- {hexs(raw)}")
            else:
                print(f"<- {msg}")


def cmd_dt1(args) -> None:
    addr = parse_hex(args.address, want_len=4)
    data = parse_hex(args.data)
    frame = build(args.device_id, CMD_DT1, addr, data)
    print(f"-> {hexs(frame)}", file=sys.stderr)
    out_name = pick(mido.get_output_names(), args.output)
    with mido.open_output(out_name) as out:
        out.send(mido.Message("sysex", data=list(frame[1:-1])))
    print("sent", file=sys.stderr)


def cmd_sweep(args) -> None:
    """Walk addresses base, base+1, base+2, ... and RQ1 each one."""
    base = int(args.base, 16)
    in_name = pick(mido.get_input_names(), args.input)
    out_name = pick(mido.get_output_names(), args.output)
    size = parse_hex(args.size, want_len=4)
    with mido.open_input(in_name) as inp, mido.open_output(out_name) as out:
        for i in range(args.count):
            # Increment the low byte of the 4-byte address. 7-bit safe: skip
            # wraps past 0x7F.
            addr_int = base + i
            addr_bytes = bytes([
                (addr_int >> 24) & 0x7F,
                (addr_int >> 16) & 0x7F,
                (addr_int >> 8) & 0x7F,
                addr_int & 0x7F,
            ])
            frame = build(args.device_id, CMD_RQ1, addr_bytes, size)
            out.send(mido.Message("sysex", data=list(frame[1:-1])))
            replies = listen_briefly(inp, args.timeout_ms)
            for msg in replies:
                if msg.type == "sysex":
                    raw = bytes([SYSEX_START]) + bytes(msg.data) + bytes([SYSEX_END])
                    print(f"{hexs(addr_bytes)}  <-  {hexs(raw)}")


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--device-id", type=lambda s: int(s, 0), default=0x10)
    ap.add_argument("--timeout-ms", type=int, default=500)
    sub = ap.add_subparsers(dest="cmd", required=True)

    rq = sub.add_parser("rq1", help="send a Data Request and print response")
    rq.add_argument("--input", required=True)
    rq.add_argument("--output", required=True)
    rq.add_argument("--address", required=True, help="4 hex bytes, no spaces")
    rq.add_argument("--size", required=True, help="4 hex bytes, no spaces")
    rq.set_defaults(func=cmd_rq1)

    dt = sub.add_parser("dt1", help="send a Data Set (write)")
    dt.add_argument("--output", required=True)
    dt.add_argument("--input", help="ignored, kept for symmetry")
    dt.add_argument("--address", required=True, help="4 hex bytes, no spaces")
    dt.add_argument("--data", required=True, help="hex bytes, no spaces")
    dt.set_defaults(func=cmd_dt1)

    sw = sub.add_parser("sweep", help="RQ1 a range of addresses")
    sw.add_argument("--input", required=True)
    sw.add_argument("--output", required=True)
    sw.add_argument("--base", required=True, help="starting 4-byte address as 32-bit hex")
    sw.add_argument("--count", type=int, default=32)
    sw.add_argument("--size", default="00000001")
    sw.set_defaults(func=cmd_sweep)

    args = ap.parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
