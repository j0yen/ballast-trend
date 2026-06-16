# ballast-trend

**Disk growth rate tracker — derivatives of ballast-survey snapshots.**

`ballast-survey` tells you what is big right now.  
`ballast-trend` tells you what is *growing*, how fast, and when you hit the wall.

## TL;DR

```
# Capture a snapshot (pipe ballast-survey output in)
ballast-survey --json --root ~/wintermute | ballast-trend snapshot

# Later, capture another one
ballast-survey --json --root ~/wintermute | ballast-trend snapshot

# See growth rates
ballast-trend report

# Machine-readable output for downstream tools
ballast-trend report --json
```

Example output:

```
ballast-trend report
  old: 2026-06-15T12:00:00  →  new: 2026-06-16T12:00:00  (24.0h interval)
  high-water mark: 95%

path                                                    new-size        delta      bytes/day  ETA(days)
---------------------------------------------------------------------------------------------------------
/home/jsy/wintermute/recall/target                        13.8G        +1.4G         +1.4G        5.7
/home/jsy/wintermute/wintermute-brain/target              14.3G        +0.5G       +500.0M       12.1
...
```

## Install

```
cargo install --path .
```

Or copy the binary:

```
cp target/release/ballast-trend ~/.local/bin/
```

## Subcommands

### `snapshot`

Reads `ballast-survey --json` from **stdin**, stamps with the current time,
and appends to a ring at `~/.local/state/ballast/trend/` (default keep-N = 30).
Oldest snapshots are pruned automatically.

```
ballast-survey --json --root ~/wintermute | ballast-trend snapshot [--now <RFC3339>] [--keep 30]
```

`--now` overrides the capture timestamp — useful for deterministic testing.

### `report`

Diffs the two most recent snapshots. Reports per-path delta bytes, bytes/day
growth rate, and ETA to a configurable high-water mark.

```
ballast-trend report [--json] [--high-water-pct 95] [--state-dir <PATH>]
```

- Growing paths get a bytes/day rate and ETA projection.
- Paths present only in the new snapshot are labeled **"new — no rate"** and
  never receive a fabricated rate.
- Shrinking paths (post-reap) show a negative delta and are **excluded** from
  ETA projections, with a note.

## Honesty constraints

- Never extrapolates a rate from a single data point (new paths).
- Reclamations (shrinking paths) are clearly labeled and excluded from ETA.
- Never walks the filesystem itself — single source of truth is ballast-survey.
- Never deletes or mutates survey snapshots.

## Part of the ballast fleet

| Tool | Job |
|------|-----|
| `ballast-survey` | Measure what is big right now |
| `ballast-trend`  | Measure what is growing and how fast ← you are here |
| `ballast-guard`  | Alert when disk hits a threshold |
| `ballast-reap`   | Free fossil build artifacts |

## License

MIT — Joe Yen
