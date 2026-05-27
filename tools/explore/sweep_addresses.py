"""One-shot sweep of candidate RE-202 address regions.

Sends RQ1 size=1 to each candidate address and reports whether the device
responds. Used to discover undocumented regions (edit buffer, setup area, etc.).
"""
from __future__ import annotations

import time

import mido


def chk(data: list[int]) -> int:
    return (128 - (sum(data) % 128)) % 128


def build_rq1(addr: tuple[int, int, int, int], size=(0, 0, 0, 1), dev: int = 0x10) -> list[int]:
    body = list(addr) + list(size)
    return (
        [0xF0, 0x41, dev, 0x00, 0x00, 0x00, 0x00, 0x18, 0x11]
        + body
        + [chk(body), 0xF7]
    )


def hexs(b: bytes) -> str:
    return " ".join(f"{x:02X}" for x in b)


CANDIDATES: list[tuple[int, int, int, int]] = [
    (0x00, 0x00, 0x00, 0x00),  # edit-buffer at 0?
    (0x01, 0x00, 0x00, 0x00),
    (0x02, 0x00, 0x00, 0x00),
    (0x0F, 0x00, 0x00, 0x00),
    (0x1E, 0x00, 0x00, 0x00),
    (0x1F, 0x00, 0x00, 0x00),
    (0x20, 0x00, 0x00, 0x00),  # spec says "MEMORY (root)" — what does this return?
    (0x30, 0x10, 0x00, 0x00),  # past slot 127
    (0x40, 0x00, 0x00, 0x00),  # Roland-convention setup area
    (0x50, 0x00, 0x00, 0x00),
    (0x60, 0x00, 0x00, 0x00),
    (0x70, 0x00, 0x00, 0x00),
    (0x7F, 0x00, 0x00, 0x00),
    (0x10, 0x00, 0x00, 0x00),  # positive control (known good)
]


def main() -> None:
    in_name = next(n for n in mido.get_input_names() if "Focusrite" in n)
    out_name = next(n for n in mido.get_output_names() if "Focusrite" in n)
    with mido.open_input(in_name) as inp, mido.open_output(out_name) as out:
        for addr in CANDIDATES:
            # Drain any pending messages (e.g. ongoing MIDI clock).
            while inp.poll() is not None:
                pass
            frame = build_rq1(addr)
            out.send(mido.Message("sysex", data=frame[1:-1]))
            deadline = time.monotonic() + 0.4
            sysex_replies: list[str] = []
            while time.monotonic() < deadline:
                msg = inp.poll()
                if msg is None:
                    time.sleep(0.002)
                    continue
                if msg.type == "sysex":
                    raw = bytes([0xF0]) + bytes(msg.data) + bytes([0xF7])
                    sysex_replies.append(hexs(raw))
            addr_s = " ".join(f"{b:02X}" for b in addr)
            if sysex_replies:
                print(f"{addr_s}  -> {len(sysex_replies)} sysex reply(s):")
                for r in sysex_replies:
                    print(f"                {r}")
            else:
                print(f"{addr_s}  -> (no reply)")


if __name__ == "__main__":
    main()
