# tools/explore

Throwaway Python scripts for reverse-engineering the RE-202 SysEx protocol.

**Not shipped.** Findings live as fixtures in [`../../re202-core/tests/fixtures/`](../../re202-core/tests/fixtures/) and as Rust types in `re202-core` once verified.

## Setup

```powershell
py -3 -m venv .venv
.\.venv\Scripts\Activate.ps1
pip install -r requirements.txt
```

On Windows, `python-rtmidi` ships precompiled wheels for recent Python versions.

## Scripts

- `capture.py` — open a MIDI input port, log every inbound message with a timestamp and hex dump. Run it, then twist knobs / save memories / switch modes on the device. Output goes to `captures/<ISO-timestamp>.jsonl`.
- `probe.py` — send a hypothesized RQ1 or DT1 and print whatever the device returns. Used for address-map sweeps.

## Workflow

1. `capture.py --list-ports` to see what MIDI ports are available.
2. `capture.py --input "RE-202" --out captures/baseline.jsonl` to start listening.
3. With the script running, perform a known action on the device (e.g. "I just turned the Echo Level knob from 0 to 64").
4. Stop the script, inspect the JSONL, identify the address bytes that changed.
5. Note the finding in `../../docs/sysex-notes.md`.
6. Add a fixture to `../../re202-core/tests/fixtures/` and a round-trip test in `re202-core`.

Captures in `captures/` are gitignored — promote anything worth keeping into the fixtures dir.
