# Task 1 Report — Remove the descent mechanic (dig gene goes inert)
Date: 2026-06-23

## Summary
Removed everything that *reads or wires* the `dig` gene (the descent mechanic),
leaving the `dig` field on `Genome` (index 9) and the `dig_drain` field on
`EcoParams` inert. Genome width stays at 10; `EcoParams` retains `dig_drain` to
avoid breaking the mapview slider (which is wired to it and cannot be touched in
this task).

---

## Changes per file

### `src/organism.rs`
- Deleted `DIG_THRESHOLD` const and its doc comment.
- Deleted `can_dig()` method and its doc comment.
- Deleted `dig_genome` test helper and `can_dig_requires_gene_above_threshold` test.

### `src/params.rs`
- Kept `dig_drain` field on `EcoParams` — it is still read by `src/bin/mapview.rs`
  (slider); removing it would break the build. The field is now inert in ecology
  (nothing reads it). Removal is deferred to a later task when the slider is also
  removed.
- Deleted the `dig_drain_default_is_present` test (field is now unreferenced by
  any ecology code, so the test had no functional purpose).

### `src/ecology.rs`
- `move_organisms`: removed `descendable: Option<&[bool]>` parameter.
- Removed `let mut best_descend = false;`.
- Removed the entire `if o.can_dig() { … }` block (layer-switch candidate).
- Removed the `if best_descend { o.energy -= eco.dig_drain; }` branch in the
  move-commit block.
- Updated doc comment to drop digger/descendable sentences.
- Removed `#[allow(clippy::too_many_arguments)]` — at 7 args clippy does not warn.
- Deleted test helpers `digger()` and `shaft()`.
- Deleted tests: `digger_descends_toward_a_rich_reservoir`, `non_digger_cannot_descend`,
  `digger_stays_without_a_descendable_column`, `digger_ascends_when_the_surface_is_richer`.
- Updated all remaining `move_organisms(…, None, None, None)` call sites in the
  test module to `move_organisms(…, None, None)`.

### `src/world.rs`
- Removed `descendable: Option<Vec<bool>>` field (+ doc comment) from `World`.
- Removed its `None` initialiser from `World::new`.
- Removed `set_descendable()` and `descendable()` methods (+ doc comments).
- Deleted `descendable_defaults_none_and_round_trips` test.

### `src/sim.rs`
- Dropped `self.world.descendable(),` argument from the `move_organisms` call in
  `Sim::step`.

### `src/mapsim.rs`
- Replaced `add_underground_reservoirs` body: removed the `desc` vec,
  `add_access_point`, and `set_descendable` calls; now registers only Underground
  sources (valaar injection; valaar stays below until a quake releases it).
- Updated test: renamed to `add_reservoirs_registers_underground_sources`, asserts
  only that the Underground source is registered and `access_points` is empty.

---

## Concern: `dig_drain` retained in `EcoParams`

The brief Step 2 says to remove `dig_drain` from `EcoParams`, but
`src/bin/mapview.rs` (out of scope for this task) still references
`eco.dig_drain` at line 353 via a slider. Removing the field causes a compile
error. The brief also says "the whole crate compiles" is a binding constraint.
Resolution: retain `dig_drain` as an inert field with an updated doc comment;
remove it in the same task that removes the mapview slider.

---

## Commands and results

```
cargo test
  → 135 passed; 0 failed; finished in 0.23s

cargo clippy --all-targets
  → Finished (no warnings)
```
