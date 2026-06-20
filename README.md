# ballast-trend

A disk growth-rate tracker that takes the derivative of [`ballast-survey`](https://github.com/j0yen/ballast-survey) snapshots: survey says what is big right now, trend says what is *growing*, how fast, and when it hits the wall.

## Why it exists

A size ranking is a photograph. It tells you the 14 GB target exists; it does not tell you whether that target has been 14 GB for a month or got there in two days. The thing you actually want to act on is the second derivative of the disk — the path that is accelerating toward the high-water mark, not the one that is merely large and stable.

`ballast-trend` gets that by differencing snapshots over time. Capture a survey now, capture another later, and it reports per-path delta bytes, a bytes/day rate, and an ETA to a configurable high-water mark. It never walks the filesystem itself — `ballast-survey` is the single source of truth — and it never fabricates a rate it cannot honestly compute.

## Install

```sh
cargo install --path .
```

This installs the `ballast-trend` binary.

## Quickstart

```sh
# Capture a snapshot (pipe ballast-survey --json in)
ballast-survey --json --root ~/wintermute | ballast-trend snapshot

# Later, capture another
ballast-survey --json --root ~/wintermute | ballast-trend snapshot

# See growth rates from the two most recent snapshots
ballast-trend report

# Machine-readable form for downstream tools
ballast-trend report --json
```

```text
ballast-trend report
  old: 2026-06-15T12:00:00  →  new: 2026-06-16T12:00:00  (24.0h interval)
  high-water mark: 95%

path                                              new-size   delta    bytes/day  ETA(days)
-------------------------------------------------------------------------------------------
/home/jsy/wintermute/recall/target                  13.8G   +1.4G        +1.4G        5.7
/home/jsy/wintermute/wintermute-brain/target        14.3G   +0.5G      +500.0M       12.1
```

## Subcommands

### `snapshot`

Reads `ballast-survey --json` from **stdin**, stamps it with the current time, and appends to a ring at `~/.local/state/ballast/trend/` (default keep-N = 30). The oldest snapshots are pruned automatically.

```sh
ballast-survey --json --root ~/wintermute | ballast-trend snapshot [--now <RFC3339>] [--keep 30] [--state-dir <PATH>]
```

`--now` overrides the capture timestamp, for deterministic tests.

### `report`

Diffs the two most recent snapshots and reports per-path delta bytes, bytes/day, and ETA to the high-water mark.

```sh
ballast-trend report [--json] [--high-water-pct 95] [--now <RFC3339>] [--state-dir <PATH>]
```

## Honesty constraints

The point of a growth tracker is to be trusted, so it refuses to guess:

- A path with only one data point gets no rate — labeled new, never extrapolated.
- A shrinking path (post-reclamation) shows a negative delta and is excluded from ETA projection, with a note.
- It never walks the filesystem; the survey snapshot is the only source.
- It never deletes or mutates a snapshot.

## Part of the ballast fleet

A family of read-mostly disk-health tools for the wintermute workspace. `ballast-trend` is the time-derivative layer over `ballast-survey`.

| Tool | Job |
|------|-----|
| [`ballast-survey`](https://github.com/j0yen/ballast-survey) | Measure what is big right now |
| **`ballast-trend`** | Measure what is growing and how fast ← you are here |
| [`ballast-guard`](https://github.com/j0yen/ballast-guard) | Watch usage against an SLO; log events; reclaim on opt-in |
| [`ballast-pilot`](https://github.com/j0yen/ballast-pilot) | Wire the guard to an hourly systemd timer |
| [`ballast-digest`](https://github.com/j0yen/ballast-digest) | Synthesize survey + trend + events into one ranked block |

## License

MIT — Joe Yen. See [LICENSE](LICENSE).
