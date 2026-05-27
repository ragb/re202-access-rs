# re202-access-rs

A SysEx codec, CLI, and WASM bindings for the [BOSS RE-202 Space Echo](https://www.boss.info/global/products/re-202/).

Modeled after [`ml10x-access-rs`](https://github.com/ragb/ml10x-access-rs).

## Crates

| Crate | Purpose | Targets |
|---|---|---|
| [`re202-core`](re202-core/) | Pure codec — SysEx framing, address map, encode/decode, YAML codec. No I/O. | native + `wasm32-unknown-unknown` |
| [`re202`](re202/) | CLI wrapping the codec. Uses midir for live device I/O. | native |
| [`re202-wasm`](re202-wasm/) | `wasm-bindgen` + `tsify` exposure of `re202-core` to JS. | `wasm32-unknown-unknown` |

## Status

Early reverse-engineering. The Roland model ID (`00 00 00 00 18`) and one address (`10 00 00 00` = System / Input Source) are confirmed from the [Electra One community thread](https://forum.electra.one/t/sysex-messages-for-boss-re-202-space-echo/2853). Everything else is being discovered against a physical device — see [`docs/sysex-notes.md`](docs/sysex-notes.md) for the running log.

## Development

```powershell
cargo test --workspace
cargo fmt --check
cargo clippy --workspace -- -D warnings
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

Dual-licensed under MIT or Apache-2.0 at your option.
