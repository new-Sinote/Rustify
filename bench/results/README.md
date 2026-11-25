# Phase 4 sanitized measurements

These files are the public, sanitized exports of the Phase 4 memory soaks. They are the data behind the tables in [`docs/PERFORMANCE_RESEARCH.md`](../../docs/PERFORMANCE_RESEARCH.md).

## Files

| File | Contents |
|---|---|
| `soak-a-low-at-launch.csv` | Soak A, `MemoryUsageTargetLevel.Low` set before navigation. Seven samples, 0–30 minutes. |
| `soak-b-hydrated-pause.csv` | Soak B, `Normal` at launch, then `Low` on a hydrated paused page, retained during playback. Seven samples, 0–30 minutes. |
| `trees/soak-a/` | Per-process PSS, working set, private commit, GPU, and CPU snapshots for Soak A. |
| `trees/soak-b/` | The same snapshots for Soak B. |

## Sanitization

The original `samples.csv` files include a `page_title` column with the track and artist that was playing at each sample. That column has been removed here. The original `smtc.csv` logs (media-session metadata), page snapshots (playlist URLs and titles), screenshots, and application logs are not published because they describe the signed-in account's listening history.

No numeric measurement was changed. The exported columns are identical to the originals except for the removed `page_title`.

## Caveats

- PSS is approximate: Windows stores a 3-bit share count, so pages shared by more than seven processes are over-attributed. See the report's method section.
- These are single-machine samples, not a benchmark suite. GPU memory and CPU vary with the GPU, DPI, and window state.
- The one-shot policy in Soak B missed the 400 MB playing budget at ad boundaries. The re-trim experiments that fix it are recorded in the report, not in these 5-minute samples.
- `Low` is a best-effort trim request, not a hard cap; private commit stays high while resident memory falls.
