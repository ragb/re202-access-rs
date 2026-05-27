# SysEx fixtures

Each `.syx` file in this directory is a raw byte capture of a single RE-202 SysEx frame, used in `re202-core` round-trip tests.

Filename convention: `<area>_<param-or-action>_<value>.syx`

Examples:
- `system_input-source_guitar.syx`
- `system_input-source_line.syx`
- `memory_001_dump.syx`

When you add a fixture, also add a Rust test in the corresponding module asserting that `Frame::decode(fixture)` produces the expected typed value AND that re-encoding round-trips.

Files in this directory are tracked in git. Don't put exploratory captures here — those go in `tools/explore/captures/` and are gitignored.
