# RE-202 SysEx notes — running log

Append-only-ish: don't delete refuted entries, mark them refuted.

## Primary spec

**[BOSS RE-202 MIDI Implementation PDF v1.00, dated 2022-03-24](https://www.zikinf.com/manuels/boss-re-202-space-echo-implementation-midi-en-78875.pdf)** (hosted on zikinf.com, a third-party MIDI-spec mirror).

This is the official Roland MIDI Implementation document — same format as their other modern Roland/Boss devices. It contains the complete SysEx address map.

**Status as of first hardware verification (2026-05-27)**: framing, RQ1/DT1 command pair, full System area (18 bytes), Memory bulk read (33 bytes per slot, MEMORY 1 at `20 20 00 00`), Tap Time nibble packing, and Identity Reply have all been device-verified against fixtures in [`re202-core/tests/fixtures/`](../re202-core/tests/fixtures/). The spec is HIGH-confidence trustworthy.

Secondary: [Electra One thread](https://forum.electra.one/t/sysex-messages-for-boss-re-202-space-echo/2853) — independently confirms framing/checksum and the Input Source address.

## Framing (confirmed)

| Field | Value | Status |
|---|---|---|
| SOX | `F0` | confirmed |
| Manufacturer | `41` (Roland) | confirmed |
| Device ID | `10`..`1F` (UI 1..16, default `10`); `7F` = broadcast | confirmed |
| Model ID | `00 00 00 00 18` (5 bytes) | confirmed |
| DT1 (write) | `12` | confirmed |
| RQ1 (read) | `11` | confirmed (round-tripped against device, both System and Memory) |
| Address | 4 bytes, each 0..=`7F` | confirmed |
| RQ1 payload | 4-byte size | confirmed. Oversized requests are truncated to the actual region size — RQ1 size `0x20` against `10 00 00 00` returned 18 bytes (full System), not 32. |
| Checksum | Roland standard, `(128 - ((sum addr+data) mod 128)) mod 128`, addr+data only | confirmed |
| EOX | `F7` | confirmed |

### Identity reply (confirmed)

Universal Identity Request (`F0 7E 7F 06 01 F7`) returns:
`F0 7E dd 06 02 41 18 04 00 00 x1 x2 x3 x4 F7`
- Device-confirmed reply: `F0 7E 10 06 02 41 18 04 00 00 00 00 00 00 F7`.
- Family code `18 04`, family number `00 00`, then 4-byte software revision.
- Observed software-revision bytes are all-zero on the tested device. Either firmware doesn't populate this field, or v1.00 reports as zeros. Not a useful firmware fingerprint on this device.
- A Roland-style DT1/RQ1 frame and this Universal Identity frame use **different** model ID forms: DT1/RQ1 carries the 5-byte `00 00 00 00 18`; the Identity Reply carries the 2-byte family code `18 04`. Both refer to the same device.

## Address space

| Prefix | Area | Status |
|---|---|---|
| `10 00 00 00` | System / global settings (18 params, ends offset `0x11`) | HIGH — bulk-read confirmed, 18 bytes |
| `20 00 00 00` | **EDIT BUFFER MIRROR** of the currently-active memory | HIGH — discovered + confirmed by device probing, 33 bytes, tracks active slot |
| `20 10 00 00` | MEMORY MANUAL | HIGH — bulk-read confirmed, 33 bytes |
| `20 20 00 00` | MEMORY 1 (33-byte block) | HIGH — bulk-read confirmed, 33 bytes |
| `20 30 00 00` | MEMORY 2 | HIGH — bulk-read confirmed (real user-programmed patch) |
| `30 00 00 00` | MEMORY 127 | HIGH — bulk-read confirmed |
| `7F 00 00 00` | unknown single byte | LOW — size 1 returns `0x00`, size > 1 returns nothing |
| Other prefixes | confirmed silent (00, 01, 02, 0F, 1E, 1F, 30 10, 40, 50, 60, 70) | swept on 2026-05-27, no response |

**Memory stride is `00 10 00 00` between consecutive slots, with carry into the high byte.** See `re202_core::address::memory_slot_address`.

## System area (`10 00 00 00`) — documented parameters

| Offset | Name | Range | Enum / notes |
|---|---|---|---|
| `00` | Input Source | 0..1 | 0=Guitar, 1=Line — round-tripped on hardware (Electra One thread) → HIGH |
| `01` | CTL1 Function | 0..4 | MEMORY UP / MEMORY DOWN / EFFECT ON-OFF / TAP / WARP |
| `02` | CTL2 Function | 0..4 | MEMORY UP / MEMORY DOWN / EFFECT ON-OFF / TAP / TWIST |
| `03` | Direct On/Off | 0..1 | |
| `04` | Direct Mode | 0..1 | 0=Analog, 1=RE-201 Simulate |
| `05` | Carryover | 0..1 | |
| `06` | Time Mode | 0..1 | 0=Normal, 1=Long. Also mirrored per-memory at `00 20`. |
| `07` | Reverb Type | 0..4 | Spring / Hall / Plate / Room / Ambience |
| `08` | Memory Extent | 1..4 | 0 excluded |
| `09` | MIDI Rx Channel | 0..16 | 0=OFF |
| `0A` | MIDI Tx Channel | 0..17 | 0=OFF, 17=RX (mirror) |
| `0B` | MIDI PC In | 0..1 | |
| `0C` | MIDI PC Out | 0..1 | |
| `0D` | MIDI CC In | 0..1 | |
| `0E` | MIDI CC Out | 0..1 | |
| `0F` | MIDI Sync Source | 0..1 | 0=Internal, 1=Auto |
| `10` | MIDI Realtime Source | 0..1 | 0=Internal, 1=MIDI |
| `11` | MIDI Thru | 0..1 | |

### Firmware-v1.10 Device-ID extension — REFUTED on this device

The [v1.10 Reference Manual](https://static.roland.com/manuals/re-202_reference_v110/eng/37135781.html) lists a **Device ID** UI parameter the v1.00 spec PDF doesn't describe. Hypothesis was that it lived at offset `0x12` in System.

Refuted (2026-05-27): RQ1 to `10 00 00 00` with size `0x20` returned **exactly 18 bytes** — the device truncates at the documented end of the region. No hidden bytes at `0x12`+ on this firmware. The Device ID UI control either lives in some other (untested) address space, or isn't exposed via SysEx at all and is set only by the front-panel mode switch.

## Memory block (per slot) — 33 bytes, offsets `00 00`..`00 20`

| Offset | Name | Range | Notes |
|---|---|---|---|
| `00` | Tape | 0..1 | 0=New, 1=Aged |
| `01` | Mode | 0..11 | 12 head-combination modes |
| `02` | Repeat Rate | 0..127 | |
| `03..04` | Repeat Rate Min/Max | 0..127 | expression-pedal range |
| `05` | Intensity | 0..127 | |
| `06..07` | Intensity Min/Max | 0..127 | |
| `08` | Echo Volume | 0..127 | |
| `09..0A` | Echo Volume Min/Max | 0..127 | |
| `0B` | Bass | 0..127 | |
| `0C..0D` | Bass Min/Max | 0..127 | |
| `0E` | Treble | 0..127 | |
| `0F..10` | Treble Min/Max | 0..127 | |
| `11` | Reverb Volume | 0..127 | |
| `12..13` | Reverb Volume Min/Max | 0..127 | |
| `14` | Saturation | 0..127 | |
| `15..16` | Saturation Min/Max | 0..127 | |
| `17` | Wow & Flutter | 0..127 | |
| `18..19` | Wow & Flutter Min/Max | 0..127 | |
| `1A` | Reverb Sw | 0..1 | |
| `1B` | Tap Sw | 0..1 | |
| `1C..1F` | **Tap Time** | 0..2000 | **Packed as 4 nibbles, MSB→LSB.** Each byte uses only the low 4 bits. value = `(a<<12)|(b<<8)|(c<<4)|d`. Range max depends on Time Mode (Normal=1000 ms, Long=2000 ms). |
| `20` | Time Mode | 0..1 | per-memory override of System `06`? Unclear precedence — TEST. |

## CC map (parameter sniffer)

The device echoes parameter changes as both DT1 (when MIDI CC Out is on) and CC. Useful for discovering which memory offset a knob writes to.

| CC | Function | Range | Mutates |
|---|---|---|---|
| 16 + 48 | Tap Time (MSB / LSB, 14-bit pair) | 0..127 each | `00 1C..1F`, but via `time = (MSB*128+LSB)*(MAX-50)/16256 + 50` with MAX = 1000/2000 ms |
| 17 | Repeat Rate | 0..127 | `00 02` |
| 18 | Intensity | 0..127 | `00 05` |
| 19 | Echo Vol | 0..127 | `00 08` |
| 20 | Bass | 0..127 | `00 0B` |
| 21 | Treble | 0..127 | `00 0E` |
| 22 | Reverb Vol | 0..127 | `00 11` |
| 23 | Saturation | 0..127 | `00 14` |
| 24 | Wow & Flutter | 0..127 | `00 17` |
| 27 | Effect On/Off | 0..63=OFF, 64..127=ON | bypass (not in memory block) |
| 82 | Tap Tempo trigger | momentary | |
| 83 | Twist | momentary | |
| 84 | Warp | momentary | |

## EDIT BUFFER MIRROR at `20 00 00 00` (undocumented, device-verified)

Not in the official MIDI Implementation PDF. Discovered by address-sweep + device probing on 2026-05-27.

- 33-byte region at `20 00 00 00` that **always reflects the currently-active memory's contents.**
- Confirmed: while the device was on MEMORY 1, RQ1 returned bytes identical to `20 20 00 00`. After advancing to MEMORY 2 via the MEMORY footswitch, the same RQ1 returned bytes identical to `20 30 00 00`. Two distinct data sets — not a coincidence.
- **Writable** (confirmed 2026-05-27 via `sync --edit`). Writing changes the live audio.
- **Volatile** — writes do NOT propagate to the active stored slot. After writing repeat_rate=100 to `20 00 00 00`, MEMORY 2 (`20 30 00 00`) was unchanged. To commit live edits, write directly to the slot's address (or have the user press WRITE on the device).

This is the ideal editor design pattern: edit buffer for live tweaks; slot writes for committing. Both first-class, independent.

Captured fixtures: [`edit_buffer_mirrors_memory2.syx`](../re202-core/tests/fixtures/edit_buffer_mirrors_memory2.syx) (snapshot while MEMORY 2 was active — identical to `memory_002_dump.syx` by design).

## Writable address map (confirmed)

| Address | Writable? | Persistent? |
|---|---|---|
| `10 00 00 00` System | yes | yes (state changes survive subsequent sessions) |
| `20 00 00 00` Edit buffer | yes | no — volatile; replaced when active memory changes |
| `20 10 00 00`..`30 00 00 00` Memory slots | yes | yes — direct DT1 persists; no "save" command needed |

Verification: 2026-05-27.
- System: wrote `MIDI Thru = 0` via DT1 to `10 00 00 11`; readback confirms.
- Edit buffer: wrote `repeat_rate.value = 100` via DT1 to `20 00 00 00`; readback confirms; MEMORY 2 unchanged.
- Memory 127: wrote `bass.value = 50` via DT1 to `30 00 00 00`; readback confirms; restored after.

Power-cycle persistence of slot writes not yet tested.

## CC stream behavior (device-observed)

With `MIDI CC Out = ON`, knob twists emit a **continuous stream of CCs** as the knob is rotated (knobs are continuous encoders, not pots — emit ~30-50 CCs per turn). Spec CC numbers all verified:

| CC | Param | Observed |
|---|---|---|
| 17 | Repeat Rate | ✓ |
| 18 | Intensity | ✓ |
| 20 | Bass | ✓ |
| 21 | Treble | ✓ |
| 23 | Saturation | ✓ |
| 24 | Wow & Flutter | ✓ |
| 27 | Effect On/Off | ✓ (left footswitch) |
| 16/48 pair | Tap Time MSB/LSB | ✓ (fires after the 3rd+ tap; pair sent atomically) |
| 82 | Tap | ✓ (right footswitch — fires 127 then 0) |

**No CC for**: MODE SELECTOR, ECHO VOL (knob exists but not yet tested), REVERB VOL (not tested), TAPE button, INPUT button, holding MEMORY button (save action).

**Side effect: TAP triggers MIDI Clock streaming.** Undocumented — not gated by `MIDI Sync Source` or `MIDI Realtime Source` System parameters. Becomes active when TAP sets a tempo and stays active until tempo is cleared.

## PC# memory mapping (confirmed)

Verified 2026-05-27 (and corrected on a second device observation):

- **PC#0 → MEMORY MANUAL**
- **PC#1 → MEMORY 1**
- **PC#2 → MEMORY 2**
- ... up to PC#127 → MEMORY 127.

The CLI's `re202 select N` sends PC#N on channel 1: `select manual` → PC#0, `select 1` → PC#1, etc.

(An earlier test in this session reported PC#0 as "unknown" — that comparison was against a stale MEMORY MANUAL snapshot taken much earlier when the device state had since changed. The device behavior is consistent with the 0-indexed convention used here.)

## Broadcast device id 0x7F (confirmed)

Verified 2026-05-27: RQ1 with `dd = 0x7F` to a known address returned a normal DT1 reply from the device. **The device responds with its own configured device id (`0x10`) in the reply**, not the broadcast id. Useful for discovery when the device id is unknown.

## Time Mode precedence (partial — independence confirmed, audio precedence open)

System Time Mode (`10 00 00 06`) and per-memory Time Mode (memory offset `0x20`) are **independent storage**:

- Wrote System = LONG via DT1; the active memory's Time Mode stayed NORMAL (no propagation).
- Wrote per-memory Time Mode = NORMAL via DT1 (also unchanged in this direction).

Which one controls the audio max-tap-time clamp (1000 ms NORMAL vs. 2000 ms LONG) was not audio-tested. Most likely the per-memory value applies when a slot is active and System applies for MANUAL — but unverified.

## What is at `7F 00 00 00`? (still opaque)

Single byte returning `0x00`. RQ1 with size > 1 returns nothing. Writing to it is risky and not yet tested. Treat as off-limits unless someone has a reason.

## Open questions (remaining)

1. **Slot persistence across power cycles**: direct DT1 writes to a slot's address succeed and are reflected by RQ1, but we haven't power-cycled the device to confirm they survive.
2. **Audio precedence of Time Mode**: System vs per-memory — which one actually clamps tap time?
3. **Where does the firmware-v1.10 Device ID setting live?** RQ1 to `10 00 00 12` returned no extra bytes. May be in `7F xx xx xx` or read-only via Identity Reply.
## Mode head combinations (confirmed from raw HTML of the reference manual)

| Mode | Heads | Mode | Heads |
|---|---|---|---|
| 1 | 1 | 7 | 1, 2, 3 |
| 2 | 2 | 8 | 1, 4 |
| 3 | 3 | 9 | 3, 4 |
| 4 | 1, 2 | 10 | 1, 3, 4 |
| 5 | 2, 3 | 11 | 1, 2, 4 |
| 6 | 1, 3 | 12 | 1, 2, 3, 4 (dense) |

Source: [reference manual "Head Combinations for Each Mode"](https://static.roland.com/manuals/re-202_reference/eng/25633275.html), parsed from raw HTML by counting `&Aacute;` markers per row to avoid summarizer ambiguity. Modes 8-12 incorporate playback head 4 (not present on the RE-201). Mode 12 has the same head set as Mode 8+12 but with head positions optimized for a denser tail.

Mirrored in code by `Mode::active_heads()` in `re202-core/src/memory.rs`.

## Refuted / dead ends

- ~~Memory slots addressed as `20 NN xx xx`~~ — refuted by the spec (stride is `00 10 00 00`).
- ~~Firmware-v1.10 Device ID lives at System offset `0x12`~~ — refuted on hardware 2026-05-27; oversized RQ1 truncates at offset `0x11`.
- ~~Edit-buffer mirror at `00 xx xx xx`, `01 xx xx xx`, `1E xx xx xx`, or `1F xx xx xx`~~ — refuted by address sweep; the mirror lives at `20 00 00 00` instead.
- ~~Roland-convention setup area at `40-70 xx xx xx`~~ — refuted by sweep; addresses silent.

## Observed device state (capture session 2026-05-27)

The reference device when we first hooked up:

- Device ID: `0x10` (UI "17", default)
- MIDI Rx Ch 1, Tx Ch 17 (= "follows RX")
- MIDI PC In/Out: ON. CC In/Out: OFF.
- **MIDI Thru: ON** — every inbound SysEx is echoed back through the device's MIDI OUT. Easy to mistake for a second device response. Set to OFF (DT1 to `10 00 00 11` value `0x00`) during reverse-engineering sessions to clean up logs.
- Input Source: Guitar
- Carryover: ON, Direct: ON / Analog, Time Mode: Normal, Reverb Type: Spring, Memory Extent: 4
- MEMORY 1: Mode 4, Repeat Rate 77, Intensity 54, Echo Vol 96, Bass 63, Treble 62, Reverb 6, Saturation 0, Wow&Flutter 82, Reverb Sw ON, Tap Time 500 ms.

These are not "factory defaults" — they reflect whatever the device had been programmed to before we started. Useful only as a reproducibility anchor for the fixtures.
