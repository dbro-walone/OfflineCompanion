# Desktop Companion MVP — Acceptance Baseline (v1)

This document is the acceptance baseline for the Desktop Companion MVP. It states
what "done" means for the runtime, interaction demo and life simulation, records
the architecture guarantees the code must satisfy, and maps every acceptance
scenario to the test that proves it.

The acceptance bar is **not** "the pet plays N animations". It is: *the pet
behaves like a stateful, personality-driven companion* — the same input yields
different behavior depending on who the character is and how it currently feels,
and every interaction flows through one observable decision pipeline.

The pipeline, verbatim from the runtime:

```
PetEvent -> PetStats (state) -> EmotionEngine (emotion) -> BehaviorPlanner (intent)
        -> BehaviorTree (action) -> per-character override -> Scheduler -> animation
```

## How to run

The acceptance suite is a pure-Rust integration test crate with no GUI, atlas or
package-catalog dependency. CI runs it as part of `cargo test --all-targets`;
locally:

```
cargo test --test acceptance_tests
```

The tests live in `tests/acceptance_tests.rs`. They drive a small `Pet` harness
that mirrors `AppRuntime::dispatch` — it advances the state engine, folds the
event into the emotion engine, feeds the fresh numbers into the behavior context,
then asks the `BehaviorController` to decide. That is the exact sequence the real
runtime runs on every event, minus the animation layer.

## Architecture validation

These properties are what make the pet a *companion engine* rather than a sprite
player. Each is asserted by at least one test and enforced by the code layout.

### 1. Engine and character are decoupled

The behavior modules contain **no character identity and no `if character == ...`
branches**. The pipeline is:

- `PetStats`, `EmotionEngine`, `BehaviorPlanner`, `BehaviorTree`,
  `BehaviorController` are all character-independent. They reason over numbers
  (energy/affinity/curiosity), an emotion space, a fixed `Personality` profile
  and a state machine — never over a character id.
- A character pack contributes **data**, not code: a `Personality` (five
  clamped dimensions), an `EmotionStyle`, a `MotionStyle`, and a
  `BehaviorMapping` of per-intent action overrides.

Proof: `identical_poke_branches_by_personality` feeds the *same* body poke to a
playful pet and a sensitive pet. The playful pet plays (`clicked`, happy); the
sensitive pet flinches (`startled`, annoyed). The fork lives entirely in the
personality data.

### 2. Behavior is data-driven and re-expressible per character

Two characters can express the same intent with different animations. The
`BehaviorTree` supplies the engine's default intent-to-action mapping; the active
character's `BehaviorMapping` may override any of the eight intents. The override
is consulted at a single point — after the tree resolves a default action id —
so the tree stays a pure, character-independent lookup.

The runtime loads a character's `BehaviorMapping` on startup, switch and reload
(`AppRuntime::apply_character_personality`), so swapping a character pack
immediately changes how intents are expressed without touching engine code.

Proof: `behavior_mapping_override_re_expresses_an_intent` sets a Play override
and confirms a head click plays the override action instead of the default
`head-pat`; `empty_behavior_mapping_keeps_engine_defaults` confirms the defaults
hold when nothing is overridden.

### 3. Characters are swappable

Switching the active character reseeds the personality (and thus the emotion
responses) and the behavior overrides from the new manifest. A v1 character pack
omits both layers and falls back to the balanced default plus the engine's
default mapping, so the original neutral behavior is preserved.

Proof: `character_package_seeds_personality_on_switch` (in `app_runtime.rs`) walks
the bundled characters and confirms each loads its own personality; combined with
the personality-driven behavior fork above, swapping a pack provably swaps the
pet's responses.

## Acceptance test matrix

| # | Scenario | Input | Expected chain | Test |
|---|----------|-------|----------------|------|
| A1 | Head pat | `PetClicked{Head,1}` | Play → `head-pat`, Playing, Playful, affinity up, Happy | `single_head_pat_plays_and_bonds` |
| A2 | Body click | `PetClicked{Body,1}` (playful) | Play → `clicked`, Playing, Playful | `single_body_click_plays_clicked_action` |
| B1 | Sensitive barrage | 3× `PetClicked{Body,n}` (sensitive) | Avoid → `startled`, Startled, Annoyed dominant | `repeated_pokes_startle_a_sensitive_pet` |
| B2 | Playful barrage | 3× `PetClicked{Body,n}` (playful) | Play → `clicked`, Playful, Happy dominant | `repeated_pokes_keep_a_playful_pet_happy` |
| C1 | Drag start + drop | `DragStarted` then `DragReleased{Drop}` | `drag` → Dragging, then `fall` → Falling | `drag_starts_and_releases_into_falling` |
| C2 | Drop landing | `DragReleased{Drop}` → `Landing` → `ActionCompleted` | `fall` → `landing` → (none) Idle | `gentle_drop_lands_and_returns_to_idle` |
| C3 | Throw + recover | `DragReleased{Thrown}` → `Landing` → `ActionCompleted` → `Tick` | `fall` (Startled) → `startled` → Recovering → `idle` | `thrown_release_startles_then_recovers` |
| D1 | Approach attached user | `PointerNear` (attachment ≥ 0.6) | ApproachUser → `look`, WaitingForResponse, Curious | `nearby_pointer_with_high_attachment_is_approached` |
| D2 | Rest when tired | `PointerNear` (energy ≤ 0.3) | Rest → `relax`, Calm | `nearby_pointer_when_tired_lets_pet_rest` |
| E1 | Seek attention | `Tick` after 60 s idle, activity > 0.65 | SeekAttention → `look`, WaitingForResponse, Curious | `restless_active_pet_seeks_attention_when_ignored` |
| E2 | Explore | `Tick`, curiosity > 0.65 | Explore → `look`, Observing, Curious | `curious_pet_explores_when_idle` |
| F1 | Sleep | `Tick` (energy ≤ 0.15) | Sleep → `relax`, Sleepy | `exhausted_pet_sleeps_and_wakes_when_restored` |
| F2 | Wake | energy restored, `Tick` | no Sleep intent, mood clears | (same test, second half) |
| G  | Celebrate focus | `PomodoroCompleted` | `celebrate`, Playing, Happy, Happy dominant | `pomodoro_completion_celebrates` |
| H  | Personality fork | `PetClicked{Body,1}` × 2 pets | playful → `clicked`; sensitive → `startled` | `identical_poke_branches_by_personality` |
| I1 | Behavior override | Play override + head click | override action instead of `head-pat` | `behavior_mapping_override_re_expresses_an_intent` |
| I2 | Default preserved | no override + head click | `head-pat` | `empty_behavior_mapping_keeps_engine_defaults` |
| —  | Observability | any event | `DecisionTrace` records state/mood/action | `decision_trace_records_the_pipeline` |

## Notes on the life-simulation model

- **Sleep vs. attention.** Energy passively drains, so a normally idling pet
  reaches the sleep threshold (energy ≤ 0.15) before the attention-seeking
  threshold (idle ≥ 60 s). In a live runtime energy is periodically restored —
  on naps and on completed focus sessions — which is what lets a well-rested but
  ignored pet seek attention. `restless_active_pet_seeks_attention_when_ignored`
  simulates that by topping up energy after the idle period.
- **Reactive vs. autonomous priority.** Reactive intents (Play, Avoid) always
  resolve, even during an open interaction session, so direct user input always
  interrupts. Autonomous intents (Rest, Sleep, Explore, SeekAttention) yield to
  an open session. This is why a pet mid-invite still responds to a click but
  will not spontaneously wander off.
- **Physical events stay in the verified arm.** Drag releases, landings and
  recovery are owned by the legacy event-to-action mapping; the planner declines
  them and lets the verified falling/edge/recovery transitions run unchanged.

## Closing the issues

- **#63 Runtime framework** — the runtime assembles the pipeline above; the
  acceptance suite exercises it end to end without a GUI.
- **#64 Interaction demo** — scenarios A–C prove clicks, the personality fork
  and drag/drop/throw all flow through the full chain.
- **#65 Life simulation** — scenarios D–G prove proximity, autonomous
  attention/exploration, the sleep/wake loop and proactive celebration.
- **#66 Acceptance baseline** — this document and `tests/acceptance_tests.rs`.
