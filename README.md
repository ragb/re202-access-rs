# re202-access-rs

A SysEx codec, CLI, and WASM bindings for the [BOSS RE-202 Space Echo](https://www.boss.info/global/products/re-202/).

Built on the shared [`midi-access-kit`](https://github.com/ragb/midi-access-kit)
foundation: the codec implements one `Device` trait and the CLI is the kit's
generic engine (same as [`ck-access-rs`](https://github.com/ragb/ck-access-rs)).

## Crates

| Crate | Purpose | Targets |
|---|---|---|
| [`re202-core`](re202-core/) | Pure codec — SysEx framing, address map, encode/decode, YAML codec. No I/O. | native + `wasm32-unknown-unknown` |
| [`re202`](re202/) | CLI wrapping the codec. Uses midir for live device I/O. | native |
| [`re202-wasm`](re202-wasm/) | `wasm-bindgen` + `tsify` exposure of `re202-core` to JS. | `wasm32-unknown-unknown` |

## Status

Codec covers the [full official MIDI Implementation](https://www.zikinf.com/manuels/boss-re-202-space-echo-implementation-midi-en-78875.pdf) plus the undocumented **edit-buffer mirror at `20 00 00 00`** (a writable 33-byte live view of the active memory, discovered by address-sweeping). System + Memory both round-trip byte-exact against device captures. The CLI is the `midi-access-kit` engine: ports/identity/dump/sync/show/lint/diff/schema/catalog/resolve. WASM bindings render typed in TS. See [`docs/sysex-notes.md`](docs/sysex-notes.md) for the running discovery log.

## CLI

The CLI addresses one document per **area** token. The RE-202 has a global
`system` area plus 33-byte memory blocks: `memory` (MEMORY MANUAL), `memory-1` …
`memory-127` (the stored user slots), and `edit` (the live edit-buffer mirror).

```text
re202 ports                                  # list MIDI ports
re202 identity                               # send Universal Identity Request; print device info

re202 dump system     -o system.yaml         # read the 18-byte System area
re202 dump memory-7   -o memory_007.yaml     # read user slot 7 (memory / memory-1..127 / edit)
re202 dump edit       -o edit.yaml           # read the edit-buffer mirror

re202 sync system     -i system.yaml         # write a YAML back to the device
re202 sync memory-7   -i memory_007.yaml --verify   # write + read-back + compare
re202 show   memory_007.yaml                 # pretty-print a YAML, identifying its kind
re202 lint   memory_007.yaml                 # validate against the typed model
re202 diff   a.yaml b.yaml                   # field-level diff between two YAML files
re202 schema system                          # print the JSON Schema (system / memory)
re202 catalog                                # print the metadata bundle as JSON
re202 resolve preset.yaml                    # normalize value names to numbers
```

Global flags: `--port "<substring>"` selects the MIDI port (both directions);
`--input-port` / `--output-port` override one direction; `--device N` sets the
channel `0..=15` (Roland device id `0x10 + N`, so the default `0` is id `0x10`).

**Dropped in the move to the generic engine** (the kit's subcommand set is
fixed): the old `dump --all` bulk loop over every slot, and `select` (a
Program-Change slot switch). Per-slot dump/sync is preserved via the `memory-N`
areas; the wasm/editor layer retains full per-slot addressing.

## Development

```powershell
cargo test --workspace
cargo fmt --check
cargo clippy --workspace -- -D warnings
```

## Library tour

A 60-second guided tour of the codec API lives at
[`re202-core/examples/library_tour.rs`](re202-core/examples/library_tour.rs).
Run it without any MIDI hardware:

```powershell
cargo run --example library_tour -p re202-core
```

It walks through: constructing an RQ1, decoding a captured fixture into the
typed `SystemArea`, decoding a `Memory` (with Tap Time auto-unpacked from its
nibble-packed wire format), `classify_inbound` routing, editing a `Memory` and
re-encoding it for a DT1 back to the device's edit buffer, and the address
arithmetic `MemorySlot` uses to handle the 7-bit carry across `User(7)` → `User(8)`.

## YAML schema

YAML files emitted by `re202 dump` carry a `# yaml-language-server: $schema=...`
header pointing at the JSON Schemas in [`schemas/`](schemas/). VS Code with
the YAML extension (and most other editors that respect the comment) will
auto-validate the document — enum values, ranges, required fields all checked.

Regenerate the schemas after changing the typed models:

```powershell
.\target\release\re202.exe schema system > schemas\re202-system.schema.json
.\target\release\re202.exe schema memory > schemas\re202-memory.schema.json
```

Building the WASM bundle:

```powershell
cd re202-wasm
wasm-pack build --target web
```

## Reverse-engineering workflow

`tools/explore/` contains throwaway Python scripts that drive the physical device over MIDI. They are NOT part of the shipped artifact:

- `capture.py` — log every inbound SysEx byte while you twist knobs / save memories. Output: timestamped JSON.
- `probe.py` — send an RQ1 to a hypothesized address, print whatever the device responds with.

Findings get promoted into `re202-core` with byte-exact round-trip tests against captured fixtures in [`re202-core/tests/fixtures/`](re202-core/tests/fixtures/).

## License

[MIT](LICENSE). See [LICENSE](LICENSE) for the full text.
