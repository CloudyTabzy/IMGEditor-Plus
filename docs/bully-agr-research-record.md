# Bully AGR and animation research record

**Status:** current local engineering record
**Updated:** 2026-09-15
**Scope:** Bully Scholarship Edition PC AGR/HXD/NIF animation assets and the
shared animation runtime in IMGEditor Plus

This document consolidates the important AGR discoveries made during the local
reverse-engineering work. The raw probe output remains in
[`bully-probe/FINDINGS.md`](../../bully-probe/FINDINGS.md), and the
handoff/checkpoint remains in
[`bully-probe/CHECKPOINT.md`](../../bully-probe/CHECKPOINT.md).

The evidence in this record comes from the locally available Bully PC corpus,
small synthetic fixtures, the local retail executable audit, and rendered pose
checks. No public description is being treated as an AGR specification. The
game files and executable are not part of the repository.

## 1. Executive summary

Bully AGR is an animation-group container, not a model format. An AGR chunk
contains compact transform keys for one clip, but it does not contain a model
name or enough information to identify the skeleton on its own. The practical
dependency chain is:

~~~text
AGR chunk(s)                         motion keys and clip durations
       │
       ├── HXD / hxds.dat            model association and clip names
       │
       └── matching NIF              hierarchy, bind pose, mesh and skin
                                      │
                                      └── NFT / texture sources
~~~

The current Rust implementation can:

- walk mixed AGR chunks and validate their declared byte spans;
- decode all six observed variants (999–1004) into normalized runtime tracks;
- preserve diagnostic information about padding, trailers, auxiliary records,
  unknown variants and malformed links;
- pair ordinary and compound HXD metadata with AGR chunks and assign real clip
  names when the evidence is sufficient;
- parse Bully NIF skin instances and deform meshes with CPU linear blend
  skinning;
- play archive AGR entries and loose `Anim/*.agr` files in the shared 3D
  animation dock; and
- bind the fully-skinned player character's 35 AGR curves to its imported NIF
  hierarchy, including action-only clips that never visit the bind pose.

The important remaining format questions are the runtime purpose of the 1002
auxiliary tail, channel 2 if it is ever emitted, and CAT/LIP/LUR relationship
work. Variants 1000 and 1001 are now decoded through their own retail-verified
record descriptors; that promotion is limited to the observed Bully PC
dialect and is not a claim about other platforms.

## 2. Corpus and dependency model

The primary evidence set was the local retail PC installation:

~~~text
C:\Games\Bully - Scholarship Edition
~~~

The useful layout is:

~~~text
<game root>\Stream\World.img     models, textures, AGR/CAT/LIP/LUR/COL entries
<game root>\Anim\                 loose AGRs, HXD files, hxds.dat
<game root>\Act\Act.img           action catalog resources
<game root>\Scripts\Scripts.img   compiled script resources
~~~

The local census recorded the following population. These counts describe one
installation and are not a universal game-version contract.

| Source | Entries/files | Relevant population |
| --- | ---: | --- |
| `Stream/World.img` | 11,980 | 550 AGR, 119 CAT, 493 LIP, 52 LUR, 5,724 NIF, 4,469 NFT, 488 COL |
| `Act/Act.img` | 479 | 479 CAT |
| `Scripts/Scripts.img` | 515 | 515 LUR |
| loose `Anim/` | 25 | 4 AGR, 20 HXD, `hxds.dat` |

The archive directory does not contain dependency edges between entries. AGR
therefore cannot be resolved by looking for an embedded model reference. The
engine's resource layer uses names and external catalogs, and mission AGRs may
contain tracks for more than one actor.

Useful known loose groups:

| AGR | Clips | First variant | Association/evidence |
| --- | ---: | ---: | --- |
| `C_Player.agr` | 439 | 1002 | `MAINPED.HXD` ownership table → `player.mxd` / `PLAYER.nif` |
| `Grap.agr` | 59 | 1002 | `MAINPED.HXD` ownership table → `player.mxd` |
| `MOT_CTRL.agr` | 418 | 1004 | direct HXD association; no ordinary mesh model |
| `NPC_Cher.agr` | 12 | 1002 | `MAINPED.HXD` ownership table → `player.mxd` |

Additional local validation assets include `SK8Board.agr`,
`AniBroom.agr`, `Bike.agr`, `AsyGate.agr`,
`Armor.agr`, `1_02_MeetWithGary.agr`, and the
mission/action fixture `Hang_Workout.agr`. `Grap.agr` occurs both loose and
in the archive; the archive copy is the same logical file with sector padding.

## 3. AGR container layout

An AGR file is a sequence of chunks. The common 20-byte little-endian header
is:

~~~text
u32 magic       = 0x00000100
u32 variant     = 999..=1004 in the observed PC corpus
u32 count       = declared record count for decoded variants
u32 reserved    = 0
f32 duration_s  > 0
~~~

The record data follows immediately. Chunks can use different variants in one
file, so a parser must not use the first file-level variant as a global
filter. The current reader finds candidate chunk boundaries by the aligned
`magic + known variant` signature and then validates each chunk's own declared
span.

For known fixed-size variants, the logical data span is exactly
`count × record_size`. Bytes after that span are not silently made into keys;
zero trailing bytes can be archive-sector padding, while non-zero trailing
bytes are reported as diagnostics. Variant 1002 has an additional runtime
auxiliary section, described below.

Variants 1000 and 1001 have a declared rotation stream followed by a
variant-specific sparse translation table. The table has no explicit count,
so the parser admits only rows whose referenced rotation index is inside the
declared stream and treats all-zero rows as archive padding.

### Container invariants

A safe reader must enforce all of the following before decoding records:

1. The input contains a complete 20-byte header.
2. The first magic and variant are valid.
3. Duration is finite, positive and within a bounded safety limit.
4. `count × record_size` uses checked arithmetic and fits the chunk slice.
5. A predecessor link points to an earlier record in the same declared
   stream, never forward or outside the stream.
6. A curve walk is bounded by the declared record count and cannot cycle.
7. Key times are finite and clamped to the clip's declared duration.
8. Quaternions are finite and non-degenerate before normalization.
9. Unknown variants remain inspectable as metadata/raw bytes rather than being
   guessed into a known layout.

The distinction between a logical chunk and an IMG sector-padded entry is
important. An archive entry may have zero bytes after its logical AGR payload;
those bytes must not create phantom clips or keys. Conversely, a real packed
record may legitimately end in zero bytes, so global zero trimming before
variant-aware parsing is unsafe.

## 4. Decoded AGR variants

### 4.1 Variant 999: float object transforms

Record size is 32 bytes:

~~~text
u16 ordinal
u16 time_norm
f32 quat_w
f32 quat_x
f32 quat_y
f32 quat_z
f32 translation_x
f32 translation_y
f32 translation_z
~~~

The quaternion is Gamebryo `(w, x, y, z)` order. The normalized time is:

~~~text
time_s = time_norm / 65535.0 × duration_s
~~~

Corpus checks against `ANIBALL` and `SK8Board` place keys on the expected
30-fps frame grid. Ordinal-zero records are default/preamble records and are
not emitted as animated keys. The decoded runtime representation exposes one
rotation channel and one translation channel for object track 0; translations
are already in metre-like source units.

Confirmed examples:

- `SK8Board.agr` contains the board's object animation family;
- the board's `IDLE`, pickup and examine clips map to HXD sequence rows; and
- float quaternion interpretation is distinguished from alternate field
  orders by the smooth full-roll behavior in the `ANIBALL` fixture.

### 4.2 Variant 1003: compact object transforms

Record size is 20 bytes:

~~~text
u16 ordinal
u16 time_norm
i16 quat_x
i16 quat_y
i16 quat_z
i16 quat_w
i16 translation_x
i16 translation_y
i16 translation_z
u16 padding_or_reserved
~~~

The compact components use a `1 / 32767` scale. The normalized time uses the
same `time_norm / 65535.0 × duration_s` equation as variant 999. The adapter
normalizes the stored quaternion into the common `(x, y, z)` representation
with a derived positive-hemisphere `w`. Translation is exposed as a linear
translation channel.

`AniBroom.agr` is the clearest validation fixture: it has three 1003 clips,
and its `LEFT`/`RIGHT` clips contain four-key sweeps that move the broom from
upright to horizontal. Their HXD durations also agree with the AGR chunks.

### 4.3 Variants 1002 and 1004: predecessor-linked packed streams

The low bits in these formats are not track/channel identifiers. They are
predecessor record indices. This was the major correction to the early probe
model.

The shared first two words are:

~~~text
word0 bits  0..10  previous_record_index
       bits 11..19 normalized_time_code (0..511)
       bit      20  qx sign
       bits 21..30 qx magnitude (10 bits)
       bit      31  qy sign

word1 bits  0..9   qy magnitude (10 bits)
       bit      10  qz sign
       bits 11..20 qz magnitude (10 bits)
       bit      21  qw sign
       bits 22..31 qw magnitude (10 bits)
~~~

Each quaternion component is signed-magnitude and scaled by `1 / 1023`.
Negative `w` keys are moved to the same quaternion hemisphere before entering
the runtime. Time is:

~~~text
time_s = min(time_code / 511.0 × duration_s, duration_s)
~~~

Records with predecessor zero start a curve. A non-zero predecessor must point
to an earlier physical record. The stream is therefore a forest of linked
chains, not a flat list of independent `(track, channel, time)` rows. The
adapter follows each chain, sorts/deduplicates keys by time, and emits a
rotation track.

#### Variant 1002

Variant 1002 records are 8 bytes and are the character rotation stream:

~~~text
20-byte header
count × 8-byte declared animation records
runtime-selected auxiliary records (also 8 bytes each)
optional four-byte trailer/padding
~~~

The declared stream convention observed across the character corpus is:

1. Record 0 is a stream-wide default identity root.
2. Later zero-predecessor roots begin authored curves.
3. The final root is an identity-only curve ending at time code 511. It is a
   runtime termination sentinel, not a bone.
4. The viewer exposes each remaining curve as numeric track
   `root_index - 1` and emits rotation channel 0.

The auxiliary tail is not currently admitted as animation data. Its size
depends on runtime/model state and cannot be reconstructed from the header
alone. Tail records can look like valid packed keys; including them creates
phantom curves and corrupts track binding. The Rust reader counts the tail,
preserves each logical 8-byte record in `AgrClip::auxiliary_tail`, and excludes
it from the curve graph. The bytes are intentionally opaque: preservation is
useful for diagnostics and future lossless tooling, but does not establish
their runtime lookup semantics.

Corpus evidence includes 507 normal loose 1002 chunks with 37 roots (default,
35 character curves, sentinel), special one-curve 1002 groups, and mission
chunks with validated declared/auxiliary splits. The reader also handles the
four-byte trailer observed in extracted/archive-backed data.

#### Variant 1004

Variant 1004 records are 12 bytes and add packed translation:

~~~text
word0: predecessor, time, qx sign/magnitude, qy sign
word1: qy magnitude, qz sign/magnitude, qw sign/magnitude
word2 bits  0..9   tx magnitude; bit 10 tx sign
       bits 11..20 ty magnitude; bit 21 ty sign
       bits 22..30 tz magnitude; bit 31 tz sign
~~~

Quaternion magnitudes use `1 / 1023`. Translation uses signed magnitude with
`0.01` units for each component. The first root is the default; the terminal
identity root is a sentinel. The remaining roots are exposed as
`root_index - 1`, with both rotation and translation channels.

The 1004 interpretation was revised after a larger corpus audit disproved an
early byte-oriented time/track hypothesis. The decisive evidence was the
retail evaluator's fixed 12-byte stride, low-11-bit predecessor lookup,
9-bit time extraction and coherent translation fields. The current decoder is
based on that packed transform model.

### 4.4 Variants 1000 and 1001: linked transform streams

These variants were promoted only after the retail executable audit supplied
both the exact record sizes and the field access pattern. The descriptor size
routines report:

~~~text
1000: count × 0x14 + metadata_count × 0x10
1001: count × 0x0c + metadata_count × 0x08
~~~

Variant 1000 uses a 20-byte linked rotation record:

~~~text
u16 previous_record
u16 time_norm
f32 quat_w
f32 quat_x
f32 quat_y
f32 quat_z
~~~

Its sparse post-key translation row is 16 bytes:

~~~text
u32 rotation_record_index
f32 translation_x
f32 translation_y
f32 translation_z
~~~

Variant 1001 uses a compact equivalent:

~~~text
u16 previous_record
u16 time_norm
i16 quat_x
i16 quat_y
i16 quat_z
i16 quat_w
~~~

The 1001 translation row is eight bytes:

~~~text
u16 rotation_record_index
i16 translation_x
i16 translation_y
i16 translation_z
~~~

Both variants use the same predecessor forest as 1002/1004. A predecessor
value of zero starts a curve; non-zero values reference an earlier physical
record. Record zero is the default root, later roots are curve starts, and a
terminal all-identity curve at the maximum 16-bit time code is treated as the
runtime sentinel. Each remaining curve becomes runtime track root_index minus
one. Translation rows attach to the curve owning their referenced rotation
record and reuse that key's time.

The scalar rules recovered from the executable and checked against the raw
corpus are:

~~~text
time_s       = time_norm / 65535.0 × duration_s
1001 quat    = signed_i16 / 32767.0
1001 vector  = signed_i16 / 1000.0
1000 quat    = full-float source values
1000 vector  = full-float source values
~~~

The first field is a predecessor index, not a bone ID. This matters for
character clips: C_Player has hundreds of 1000/1001 records but only a few
dozen animation curves. The translation table has no explicit count in the
chunk. The reader scans only complete descriptor-sized rows, admits a row
when its non-zero rotation-record index is inside the declared stream, and
reports malformed non-zero rows. Rows are treated as a contiguous table: the
first all-zero padding row or malformed non-zero index closes admission, and
later record-shaped bytes remain ignored. This prevents coincidental words in
sector padding or a trailer from becoming a translation while still preserving
a valid final row whose vector components end in zero bytes.

The strongest local confirmations are C_Player.agr clip 13 (1001, 382
rotation records and seven translation rows), clip 377 (1000, 442 records and
25 translation rows), and clip 438 (1000, 237 records and 12 translation
rows). The broader archive census found the same strides, valid predecessor
forests and in-range metadata indices across the observed 1000/1001
population. This is confirmed for the Bully PC dialect only; no other
platform's AGR dialect is implied.

### 4.5 Channel policy

The normalized runtime currently has:

~~~text
channel 0 = rotation
channel 1 = translation
channel 2 = reserved/withheld; semantics not established
~~~

Translation is known for 999, 1003 and 1004. Character 1002 records account
for predecessor, time and quaternion bits only; they do not contain root
translation. A future channel-2 discovery must be independently validated
before it is treated as scale, visibility, material or any other property.

### 4.6 Single-frame held-pose stubs

Compound files can contain short chunks that are not animations in the
practical sense, and they should be recognized by shape rather than treated
as parse or naming failures. The `Area_GirlsDorm` AGR shows the pattern
(2026-09-15 probe of all 15 chunks):

- Five of the six uncovered chunks (5, 10, 12, 13, 14) are each 628 bytes:
  the declared stream holds exactly two keys per curve, at `t = 0.000` and
  `t = 0.033 s` — one frame at the game's 30 fps — so every curve is a held
  value.
- The sixth (clip 11, 636 bytes) spans two frames (0.067 s) with a token
  third key on one curve: a micro-hold, still effectively static.
- They are distinct poses, not byte duplicates: pairwise diffs run 80-124
  u32 words, so each stub freezes a different pose (seated, lying, tool-in-
  hand variants and similar placeholders).
- The catalog omits them because nothing moves; the nine named rows in
  `MAINPED.HXD` cover only the authored actions.

Recognition rules distilled from this case: a chunk whose curves each carry
a single hold pair (at most two keys) is a static pose stub, and a chunk of
at most two frames with a stray third key is a micro-hold. Both play
correctly as frozen frames, legitimately have no catalog name, and must
keep the positional `clip_NN` label rather than a guessed one.
`Area_GirlsDorm` clip 5 and clips 10-14 are the reference examples.

## 5. HXD and `hxds.dat`: association and naming

AGR has no useful embedded model name in the observed corpus. The association
layer is outside the AGR bytes:

- loose `Anim/<MODEL>.HXD` records describe actor, vehicle and prop groups;
- `Anim/hxds.dat` contains concatenated `ANIM + body_length + body` records for
  level objects; and
- `MAINPED.HXD` contains a compound external-resource table for character
  groups.

The two catalogs are disjoint in the local installation. `hxds.dat` contains
130 level-object records; the loose HXD set contains 20 actor/vehicle/weapon
records. The body is a compiler/heap dump rather than a clean portable schema:
serialized pointer-looking values, `0xCD` fill, and even a stale editor-dialog
string are present. Only the fields that survive cross-file checks are used.

### Reliable HXD fields

The current reader safely extracts:

- model name: an identifier after the last `01 00 00 00` marker, with a file
  stem fallback for `MAINPED.HXD`;
- ordered namespaced sequence strings such as
  `SKATEBOARD\1_07_PICKUP`;
- sequence duration and blend weight from the preceding floats; and
- for `MAINPED.HXD`, a duplicated sequence descriptor containing AGR encoded
  size and external-resource index.

The HXD sequence order is a strong relation to AGR chunk order:

| Pair | Evidence |
| --- | --- |
| `SK8Board.agr` ↔ `SK8BOARD.HXD` | 17 sequences and 17 AGR chunks; durations agree |
| `AniBroom.agr` ↔ its HXD record | 3 sequences and 3 chunks; IDLE/LEFT/RIGHT durations agree |
| `MOT_CTRL.agr` ↔ direct HXD | 418 sequence/chunk rows; old 386 count was an incomplete probe |

Ordinary records use exact sequence-count equality before names are applied.
Compound `MAINPED.HXD` records are different:

1. Select sequence rows by the external resource index.
2. Require the two duplicated descriptors to agree. The copies must be read
   from the row's 32-byte name *field start*, not from wherever the name
   string was found: fused float-tail bytes (`8MINISNOW\...`) and multi-word
   namespaces (`N2B DISHONERABLE\...`) shift the found position by up to
   four bytes either way. The reader pins the field by trying the found
   position first (so clean rows keep their exact values) and then the
   nearest offsets outward, requiring plausible duration/weight floats, a
   bounded chunk size and an in-range resource index.
3. Align rows to AGR chunks by encoded chunk size, with duration only as a
   tie-breaker.
4. Allow only the bounded, four-byte-aligned final padding discrepancy found
   in the retail catalog.
5. Reject rows that fail every candidate instead of guessing a name.
6. Accept partial coverage: a resource may list fewer rows than the AGR has
   chunks, and the aligned run still names its covered clips while the rest
   keep the positional `clip_NN` label (`Area_GirlsDorm` names 9 of its 15
   chunks — the six single-frame filler clips have no catalog rows — where
   the earlier all-or-nothing rule dropped every name for the whole file).

This guarded alignment names all 439 `C_Player.agr` clips and all 59 `Grap`
clips. Namespace matching alone is insufficient because one AGR can contain
sequences from several namespaces and a namespace can occur under more than
one external resource.

The earlier note that `DISHONERABLE\VAULT_BAR` was a "malformed stale row"
was wrong and is retracted: the row is valid (size 7440, resource 97) as is
`MINISNOW\MINISNOW_HITSHVL` (size 4344, resource 95). Both were dropped by
the descriptor-offset bug described in point 2, so `N2B Dishonerable` clip
00 and `W_snowshwl` clip 00 stayed unnamed even though the catalog names
them. Correcting the field pinning recovered both rows and changed nothing
else in the 3,359-row catalog — verified by a full per-resource diff of
every owned sequence before and after the change.

A full `World.img` naming census (2026-09-15, via
`agr_corpus_audit_when_requested`) now finds **one** paired AGR with any
unnamed clip; every other pair is fully named (or never reaches naming
because the HXD-first pairing only selects files with a catalog presence):

| AGR | named | unnamed content |
| --- | --- | --- |
| `Area_GirlsDorm` | 9/15 | five one-frame held poses plus one two-frame micro-hold (§4.6) |

Guessing a
neighbouring row's name would be worse than an honest blank, so the
alignment's no-guess rule stands: an uncovered real animation is reported
as `clip_NN`, not named. The census also classifies each unnamed clip in
the audit output (`stub`/`micro`/`motion` counts per pair) so a future
catalog change that starts hiding real motion is visible at a glance.

HXD joint strings are useful evidence for prop rigs, but the semantic names in
`MAINPED.HXD` do not directly name the imported player NIF nodes. They cannot
be used as a shortcut for the player bone binding problem.

## 6. NIF hierarchy, skinning and AGR binding

### 6.1 Skin data contract

For a skinned Bully NIF, the runtime follows this chain:

~~~text
vertex influence row
  → partition bone-index slot
  → partition palette entry
  → NiSkinInstance bone reference
  → NiNode / runtime NodeId
~~~

The `NiSkinData` per-bone `SkinTransform` supplies the mesh-local inverse bind
transform. The validated partition layout is:

~~~text
u16 num_vertices, num_triangles, num_bones, num_strips,
    num_weights_per_vertex
u16 palette[num_bones]
u8  has_vertex_map       → u16 vertex_map[num_vertices]
u8  has_vertex_weights   → f32 weights[num_vertices][width]
u16 strip_lengths[num_strips]
u8  has_faces            → u16 triangles[num_triangles][3]
u8  has_bone_indices     → u8 bone_indices[num_vertices][width]
~~~

There is no hidden count before each weight row. The `has_faces` byte is real;
an independent Python parser in the local tooling omitted it and compensated
by reading the next flag at the wrong width. That mistake happened to make
some weight rows appear correct while shifting partition triangles. The Rust
reader follows the raw retail bytes and keeps the direct `NiSkinData` weight
rows as a fallback when a valid partition is absent.

The full local NIF audit found 2,613 skin instances across 5,725 files. It
found no invalid bone references, malformed palettes, non-finite weights,
conflicting partition overlaps or skeleton-root violations. Seventy-three
instances omit `NiSkinPartition` and use direct `NiSkinData` weights. On the
player model, four skinned meshes account for 2,587 influenced vertices.

The pose evaluator is CPU linear blend skinning:

~~~text
posed_vertex = Σ weight × (view × world[joint] × inverse_bind) × bind_vertex
~~~

The CPU path is the correctness oracle. GPU skinning remains an optional
optimization and is not needed to understand AGR semantics.

### 6.2 Why the first player animation was twisted

The initial assumption was that AGR's numeric curve order directly matched the
NIF depth-first node order. On the player this produced the characteristic
“neighbor rotation” failure: bone positions could look plausible while the
skinned body twisted around the spine, hands and legs.

The investigation eliminated several misleading signals:

- bone lengths cannot detect a rotation permutation;
- a hierarchy direction test in a single composed model frame accepts any
  consistent local-rotation assignment;
- HXD semantic joint names do not name the player NIF's `Root ...` nodes; and
- a single clip may hold a joint far from its rest rotation throughout the
  clip.

The current generic solution is cross-clip bind-pose calibration:

1. Aggregate valid rotation keys for each AGR target over the whole library.
2. Score a candidate node by the closest quaternion angle to its NIF rest
   rotation.
3. Solve an ordered, one-to-one assignment instead of greedy nearest-neighbor
   matching.
4. Exclude mesh nodes and, for skinned models, derive candidates from skin
   joints and their valid ancestors/attachments.
5. Use confident numeric matches only as an order-offset prior; the prior
   cannot admit a candidate that fails the raw angle threshold.
6. Leave unmatched tracks explicitly unbound with diagnostics.

This fixed ordinary `C_Player`/`PLAYER.nif` clips and keeps the calibration
stable across clip selection.

### 6.3 The action-only root/torso failure

The remaining failure appeared in `Hang_Workout.agr`, especially the
`JOCK_PSHUP_IN` and `JOCK_PSHUP_LOOP` actions. The feet and hands moved locally,
but the body did not rotate into the floor-facing push-up pose. The problem
was not a missing foot mesh or an AGR quaternion axis issue. The strict
calibrator intentionally refused three curves because those action clips
never brought their root/torso rotations close enough to the NIF bind pose.
Those curves were therefore left at rest, so the child limbs animated under a
standing torso.

The local fixture contained seven observed clips, all variant 1002 with 35
rotation tracks. The missing tracks were the root-chain entries:

~~~text
AGR track_000  → NIF track_001  Root
AGR track_008  → NIF track_009  Root01
AGR track_009  → NIF track_010  Root Spine
~~~

The imported player hierarchy has a synthetic scene root and a non-animated
dummy before the actual animated skeleton:

~~~text
Scene Root
└── track_000       Dummy helper
    └── track_001   Root
        └── track_002   Root Pelvis
            ├── track_003 ... left leg chain
            └── track_006 ... right leg chain
~~~

The AGR character stream has 35 curves and omits the synthetic NIF scene root;
the importer exposes the corresponding animated nodes as `track_001` through
`track_035`. Thus the proven player relationship is:

~~~text
AGR track_i  →  imported NIF track_(i + 1)
~~~

The character stream is the same semantic list on every verified rig: the
skeletons' animated nodes in imported order, starting at the semantic
root's first child. The `Dummy` placeholder is never an animation target;
sibling wrappers only shift the normalized numbering. The recovery is
deliberately narrow and allowed only when all of the following hold:

- the model is actually skinned and Z-up;
- the animation library carries the Bully AGR provenance;
- targets form a contiguous `track_000...` sequence;
- the preserved `Dummy` node is the semantic root: a direct child of
  `Scene Root` whose source name survives import;
- the covered run selects unique non-mesh skin candidates, and the first
  curve targets the placeholder's first child.

When those checks pass, the structural mapping recovers action-only root and
torso tracks, producing 35/35 bindings and allowing the push-up body rotation
to propagate into the legs and arms. If any check fails, the generic
calibration remains in place and the track stays honestly partial. This is a
Bully importer invariant, not a general-purpose numeric retargeter.

A 2026-09-15 correction pass produced the current rule after two failed
intermediate states. First, an offset re-anchored to the semantic root was
applied to every rig; because the player's `Dummy` occupies `track_000`
itself, that bound the first player curve to the placeholder and regressed
`Hang_Workout` and `RAT_PED` while the Mandy gate stayed green. Second, the
interim fix kept a "root-inclusive" variant for wrapper rigs, which left
Mandy bound at `+2` and visibly twisted in the GUI. Probing the Mandy
library against node rests settled it: the rigid facial chain (curves 14-17
for Ponytail1/EyeLids/Brow/Eyes on the shared export order) matches
`track_017..track_020` at ~0.0 degrees, i.e. exactly `+3` for her numbering,
while the `+2` reading maps curve 14 to `Head` with a >50-degree mismatch.
The stream therefore always skips the placeholder, and the offset is simply
the semantic root's normalized index plus one (`PLAYER`/`RAT_PED`: `+1`;
wrapper-heavy `JKGirl_Mandy`: `+3`). The verified run supersedes the pose
calibration under its signature; there is no count-based override.

#### Wrapper-heavy character rigs: Mandy

`1_08_MandPuke.agr` paired with `JKGirl_Mandy.nif` exposed a second, easily
missed hierarchy shape. The visible character is accompanied by wrapper nodes,
while the actual AGR skeleton begins at a source node named `Dummy`. The
relevant source-name associations are:

~~~text
visible wrapper path:   JKGirl_Mandy → __NDL_MultiMtl_Node → body shape
animation path:         Dummy → Root → Root Pelvis → legs, spine and arms
preview helper path:    ARROW → Editable Poly
~~~

The normalized NIF names put `Dummy` at `track_002` behind the sibling
wrapper chain, so this rig uses `AGR track_i → NIF track_(i + 3)`; the
ordinary player rig uses `+1` because its `Dummy` occupies `track_000`
itself. Both are the same rule: the stream lists the skeleton from `Root`
onward and never animates the placeholder. The offset was pinned by unique
~0.0-degree rest matches on the rigid facial chain (curves 14-17 →
`track_017..track_020`); the earlier placeholder-inclusive `+2` reading
mapped curve 14 to `Head` with a >50-degree mismatch and twisted the whole
skinned body. The 1001 sparse translation rows ride curve 0, which under
this stream addresses `Root`. The real Mandy regression binds all 35
rotation curves and all 36 property tracks, including the right arm, and the
gate now asserts the `+3` run plus the facial-chain, `Root` and `ARROW`
anchors.

**Milestone (2026-09-15, GUI-validated):** with the corrected stream the
`MANDY_PUKE_LOOP` clip renders as a coherent puke loop with floor contact in
the app — the wrapper case is closed, and the same run keeps the player
family and `RAT_PED` at their verified `+1` order.

#### Wrapper-offset classes across the corpus (2026-09-15 audit)

A full `World.img` sweep (`agr_corpus_audit_when_requested`; 550 AGRs, 433
paired, 280 character-shaped) shows the placeholder-skip rule holds on every
pair — the semantic root just sits behind a different number of wrapper
branches per model, so the effective offset varies but never the rule:

| offset | pairs | representative models |
| --- | ---: | --- |
| `+1` | 193 | `PLAYER.nif` (Jimmy and the player mission/action libraries), `RAT_PED`, the plain ped rigs |
| `+2` | 43 | `DOH3a_Gurney`, `GN_Sexygirl`, `TO_Business1`, `Nemesis_Gary`, `NDH1a_Algernon`, `GRH3a_Ricky` |
| `+3` | 38 | `JKGirl_Mandy`, `JKGirl_MandyUW`, `PRGirl_Pinky`, `GRGirl_Lola`, `TO_Cop`, `TE_Art`, `Player_Mascot`, `TO_Oldman2` |
| `+5` | 2 | `bike.nif`, `SCOOTER.nif` (four wrappers before an uppercase `DUMMY`) |

The sweep also exposed a rendering bug from the Mandy hardening: the
helper-mesh suppression hid every mesh named `Editable Poly`, but that 3ds
Max default name is also used for the *body* of several ped NIFs (for
example `DOH3a_Gurney`'s skinned 32-joint body), so the `+2`/`+3` classes
rendered empty. The rule now hides `Editable Poly`/`Mesh` only when the mesh
is unskinned; the existing `ARROW`-parent and six-vertex checks still cover
Mandy's helper geometry.
`wrapper_ped_body_stays_visible_when_available` pins the regression.

Known sweep leftovers (honest partials, no twist risk): three 2-track
weapon AGRs (`BATON`→`bat.nif`, `BROCKETL`→`rock.nif`,
`Slingsh`→`slingshot.nif`) bind 0/2 through the rest-angle admission; the
`V_*` vehicle mission groups (`V_Bike` 12/35, `V_COPBIKE` 9/35,
`V_SCOOTER` 12/35) and the nonsensical heuristic pairs
(`2_S02CharSheets`→`charSheet.nif`, `2_06MovieTickets`→`ticket.nif`) stay
partial and mostly need association/pairing work rather than binding work.

The NIF also contains axis/arrow helper meshes. They remain in the hierarchy
for skin and binding validation, but are marked preview-only: they do not draw,
do not contribute to posed bounds or floor placement, and retain an empty
scene index list so the animated mesh-cache order remains stable.

### 6.4 Cross-rig 1004 binding evidence

The object/prop side now has a separate acceptance gate from the player
calibration. Four same-stem archive pairs were checked through the exact
runtime binding path:

| AGR | NIF | 1004 clips checked | Result |
| --- | --- | ---: | --- |
| AsyGate.agr | AsyGate.nif | 3 | every emitted rotation and translation track bound |
| Armor.agr | Armor.nif | 4 | every emitted rotation and translation track bound |
| Bike.agr | bike.nif | 3 | every emitted rotation and translation track bound |
| SK8Board.agr | SK8Board.nif | 2 | every emitted rotation and translation track bound |

These pairs establish that the curve-root-minus-one numbering used by the
1004 adapter agrees with the importer’s BonesOnly NIF order across more than
one prop hierarchy. The regression test is
object_1004_curve_roots_bind_across_matching_rigs_when_available and is gated
by IMGEDITOR_BULLY_STREAM so game data stays outside Git. This is evidence for
the observed Bully PC object rigs, not permission to apply the numbering to
unrelated skeletons or console dialects. A future named-joint resolver can
still improve diagnostics, but it is no longer required for these validated
same-stem prop pairs.

### 6.5 Root motion and floor placement

Character 1002 clips are rotation-only in the observed data. No root
translation field is missing from the packed record: its bits are accounted
for by predecessor, time and quaternion. The runtime/game code grounds the
actor separately. `MOT_CTRL.agr` is a 1004 object-transform group and does not
index-match `C_Player.agr` as a hidden player root-motion source.

The viewer therefore keeps source rotation semantics and offers a presentation
policy that samples a clip at bounded, dense uniform times, finds its lowest
visible deformed vertex, and applies one constant grounding offset for the
whole clip. The measurement converts posed viewer-space vertices back into
source space before projecting onto the source ground normal, then transforms
the correction back into viewer space. This is important for Z-up Bully assets:
applying a source-space Z correction directly to the Y-up viewer previously
shifted the character along depth. The correction is measured together with
the rest-pose centering offset, so a centered prone clip is both centered and
planted. Changing the viewer's centered/world origin mode re-samples the same
clip in the new display frame, so the toggle cannot reintroduce that shift.
Initial camera framing uses the grounded clip envelope rather than only the
standing rest bounds.

A per-frame offset was rejected because it causes visible bobbing/yanking
during falls and transitions. This policy makes prone clips inspectable
without claiming to reproduce the game's actor-placement code. Helper meshes
such as the Mandy axis/arrow geometry are excluded from both the lowest-point
measurement and the rendered scene.

### 6.6 Facing conventions: why played clips may show the character's back

A recurring observation: the static NIF preview shows PLAYER.nif facing the
camera, while many played clips show the character from behind. A
signed-facing probe (`facing_conventions_when_available`, over
PLAYER.nif + C_Player.agr) measured the front direction — the thin horizontal
principal axis of the posed vertex cloud, signed by the ankle-to-sole offset
so the toes define "front" — and established:

- The static preview and the animation rest scene face identically
  (0.21 vs 0.29 deg from view +Z). There is exactly one display convention:
  both paths apply the same `Zup -> Yup` matrix and the same camera reset
  (`yaw = 0`, camera on view +Z). The viewer never yaws one path relative to
  the other.
- The bind pose's front points toward the default camera: source **-Y**
  maps to view +Z (source +Y maps to view -Z by design so the depth axis
  aligns with the view).
- Every one of C_Player's 439 clips keeps the body on that same source ±Y
  facing line (zero sideways), but the clip data itself splits roughly evenly
  between facing -Y (211 clips) and +Y (228 clips) at t=0.

The split is a format property, not a binding or display defect: AGR clips
are authored relative to the game's actor node — the `Dummy` placeholder the
stream never animates — and the engine composes the actor's world facing on
top. The bind pose has no reason to match any clip's authored facing, and in
Bully it happens to face the opposite way from about half of them. A played
clip showing the character's back at the default camera is therefore
faithful to the data; orbiting the camera (or enabling follow-root during
motion) recovers the front view.

## 7. Runtime implementation map

The format adapter and shared player are intentionally separate:

| Area | Current responsibility |
| --- | --- |
| [`src/inspector/animation/bully.rs`](../src/inspector/animation/bully.rs) | AGR chunk parsing, six decoded variants, HXD-independent library conversion, NIF model bridge |
| [`src/inspector/animation/hxd.rs`](../src/inspector/animation/hxd.rs) | loose HXD and `hxds.dat` records, `MAINPED` ownership alignment, model/clip naming |
| [`src/inspector/animation/clip.rs`](../src/inspector/animation/clip.rs) | normalized tracks, key validation, sampling |
| [`src/inspector/animation/binding.rs`](../src/inspector/animation/binding.rs) | exact binding, ordered calibration, guarded Bully action-only recovery |
| [`src/inspector/animation/model.rs`](../src/inspector/animation/model.rs) | validated hierarchy, bind pose, skin assets and node transforms |
| [`src/inspector/animation/pose.rs`](../src/inspector/animation/pose.rs) | hierarchy evaluation, CPU skinning, helper suppression and source/view-space clip grounding |
| [`src/ui/viewer_session.rs`](../src/ui/viewer_session.rs) | persistent animation session, calibration lifetime, transport, crossfade and pose revisions |
| `src/ui/view.rs` / `src/ui/app.rs` | clip dock, loading/pairing actions, diagnostics and user controls |
| `src/inspector/scene3d/headless.rs` | deterministic headless render/probe path for pose regressions |

Calibration is built once per model/library session and reused for clip
selection. Invalid clips are rejected before sampling. Invalid model
transforms, malformed skin records, non-finite values and unsafe counted
arrays fail closed rather than reaching the renderer.

Animated scenes carry each mesh's diffuse texture name from the AGR model
build (`bully::model_from_nif*` reads the shape's `NiTexturingProperty`,
falling back to the NIF-wide first texture) and the loader resolves pixels
through the static preview's three-tier resolver (NFT catalog → archive
texture → loose file), so `Textured` and `Alpha blend` behave identically
in both viewer paths. The resolver keys its NFT lookup by the model *stem*
(`PLAYER`), never the entry file name (`PLAYER.nif`, which would probe
`PLAYER.nif.nft`); a corpus gate pins this by resolving every `PLAYER.nif`
diffuse to pixels through the shared `nif_texture_resolver`. The animation
dock's **Model** picker re-plays a retained AGR request on any
catalog-associated archive model (loose HXD stems + `hxds.dat` + `MAINPED`
resources resolved to NIF entries) and recomputes the binding badge per
selection; stale loads are dropped by a monotonic serial and an archive
generation/name guard. While a re-play is active and the animation entry
stays selected, the Texture tab follows the picked model: the completion
publishes the model's decoded companion textures under the model entry and
pins it, so switching models swaps the viewport and the previews together.

## 8. Validation record

The current implementation has both synthetic and local-corpus coverage.

### Structural and parser checks

- 554 local AGR files and 3,261 clips were structurally scanned with no
  reported structural errors in the validated six-variant population.
- Linked 1000/1001/1002/1004 predecessor links are checked for
  forward/out-of-range references, branches, cycles and decreasing key times.
- Fixed-size variants are bounded by declared counts; 1000/1001 metadata rows
  are admitted only with in-range record references, the first padding or
  malformed row closes the table, and archive padding is not treated as
  records.
- Real HXD duration/order and compound ownership checks cover `SK8Board`,
  `AniBroom`, `MOT_CTRL`, `C_Player`, `Grap` and `NPC_Cher`.
- AGR parser fixtures cover bad magic, unsupported variants, bad durations,
  record overflow, misalignment, packed-link failures and valid zero-ended
  records.

### Rendering and binding checks

- `SK8Board.agr`: board object motion and named clips.
- `AniBroom.agr`: compact 1003 sweep motion.
- `Bike.agr`: object animation with partial track coverage reported honestly.
- `1_07_Sk8Board.agr`: mission/action 1002 preview with partial model binding.
- `C_Player.agr` + `PLAYER.nif`: 35/35 character curves, CPU LBS, RUN/STRAFE/
  ground-state pose checks and rest-bridge agreement.
- `Hang_Workout.agr` + `PLAYER.nif`: action-only root recovery, including
  push-up root/torso propagation and the leg/hand binding regression.
- `1_08_MandPuke.agr` + `JKGirl_Mandy.nif`: wrapper-aware `+3` binding that
  skips the `Dummy` placeholder, helper-mesh suppression, source/view floor
  conversion, grounded framing and right-arm deformation coverage; the
  `MANDY_PUKE_LOOP` render was GUI-validated on 2026-09-15.
- `RAT_PED.agr` + `rat_ped.nif`: ordinary character rig keeping the exported
  one-node offset (`curve i → track_(i + 1)`), never binding the `Dummy`
  placeholder.

The latest rendering and binding coverage also includes the full-float 1000
and compact 1001 streams from C_Player.agr, plus same-stem 1004 binding across
AsyGate/AsyGate.nif, Armor/Armor.nif, Bike/bike.nif and
SK8Board/SK8Board.nif.

The core test names that encode the latest lessons are:

- `calibrated_binding_matches_bind_pose_when_available`;
- `bully_action_only_tracks_recover_the_imported_numeric_offset`;
- `player_family_binding_keeps_the_imported_order_when_available`;
- `action_only_player_clips_recover_root_tracks_when_available`;
- `ordinary_character_rig_keeps_the_exported_order_when_available`;
- `wrapper_rig_stream_skips_the_placeholder`;
- `wrapper_ped_body_stays_visible_when_available`;
- `compound_resource_partial_naming_when_available`;
- `compound_fused_prefix_rows_recover_when_available`;
- `catalog_model_candidates_resolve_when_available`;
- `model_build_reads_diffuse_texture_names`;
- `agr_textures_resolve_for_models_when_available` (every `PLAYER.nif`
  diffuse resolves to pixels through the shared resolver);
- `texture_tab_follows_the_played_model_while_an_agr_replays`;
- `facing_conventions_when_available` (static preview == animation rest;
  bind front = source −Y toward the default camera; C_Player's 439 clips
  all keep the ±Y facing line while splitting ~evenly between −Y and +Y —
  see §6.6);
- `partial_coverage_names_only_matched_clips`;
- `numeric_recovery_requires_the_verified_dummy_identity`;
- `stepping_never_stalls_at_grid_rounding`;
- `focus_loss_cancels_an_active_drag`;
- `invalid_clips_are_never_sampled`; and
- `non_finite_or_degenerate_node_transforms_are_rejected`;
- `ground_offset_projects_view_pose_back_into_source_space`; and
- `mandy_puke_grounding_and_right_arm_when_available`.

The new decoder and cross-rig gates are:

- linked_1000_and_1001_decode_with_sparse_translations;
- object_1004_curve_roots_bind_across_matching_rigs_when_available.

Real-corpus gates are environment-dependent and must not embed game data in
Git. The local paths and probe scripts are described in the checkpoint.

## 9. Related Bully formats: current boundaries

These files are related to the AGR workflow but are not AGR variants:

| Format | Established role | Current state |
| --- | --- | --- |
| HXD / `hxds.dat` | animation hierarchy/catalog and AGR association | model pairing and guarded clip naming implemented |
| NIF | model hierarchy, mesh, materials and skin data | static and animated CPU-skinned preview implemented |
| NFT | Bully/Gamebryo texture catalog/payload | texture preview and source resolution implemented |
| CAT | compiled action tree/catalog | strings and relationship research only; no safe typed parser yet |
| LIP | lip-sync records with `Speech.bin` references | record shape and reference offsets observed; payload semantics deferred |
| LUR | compiled Lua 5.0-era script bytecode | identified; future read-only script inspector, never execution |

Useful local observations that must remain attached to the roadmap:

- CAT strings can reveal action paths, resource paths and logical `.act`
  names, but a readable path is not proof of a typed node or an AGR link.
- LIP starts with a 16-bit count and uses a 0x18-byte record stride in the
  local reference tooling. Observed fields at `+0x00`, `+0x0C`, `+0x10` and
  `+0x14` maintain Speech entry index/offset/size/hash relationships; the
  remaining words and compressed payload are not decoded.
- LUR files begin with the Lua 5.0 binary signature (`\x1bLuaP`) in the local
  Scripts corpus. Header flags must be read from the chunk; a hard-coded Lua
  host layout is unsafe.
- No PC AGR result should be generalized to Xbox 360, PS2, Wii, PSP, mobile or
  Anniversary resources without a real fixture. Shared extensions do not prove
  shared byte layouts.

The long-form work plan is
[`bully-agr-cat-lip-lur-roadmap.md`](bully-agr-cat-lip-lur-roadmap.md). Its
older phase prose is being reconciled with this record; the tracked checklist
is [`TODO.md`](../TODO.md).

## 10. Lessons learned for researching AGR

### Start with a corpus census, not a format guess

Count files, variants, durations, sizes, archive locations and duplicate
copies before assigning meaning to a field. The first AGR observations made
the second word look like a version and the third word like a universal count;
the expanded corpus showed mixed chunks, auxiliary data and multiple resource
families. A census prevents a convenient sample from becoming a false rule.

### Separate framing from semantics

First establish where a chunk begins and ends. Only then interpret record
fields. The 1002 tail and IMG sector padding demonstrate why “read until the
next plausible header” and “trim all zeros” are dangerous unless bounded by
the surrounding archive/chunk context.

### Treat linked streams as graphs

The low 11 bits looked like a track field until the retail evaluator and corpus
showed predecessor chains. For packed data, build a bounded graph, check
forward links/branches/cycles, then recover curves. Never flatten an index into
an identity merely because its numeric range looks convenient.

### Use orthogonal evidence

A convincing interpretation should agree across at least two independent
signals: executable access pattern, record stride, key monotonicity, duration
alignment, HXD sequence order, model pose, or exact byte sizes. A visual match
alone is especially weak because neighboring rotations can still produce
plausible silhouettes.

### Keep confidence labels explicit

Use three practical labels in notes and diagnostics:

~~~text
confirmed       repeated invariant across corpus or direct runtime evidence
strong          independent cross-check, but limited to a resource family
open            plausible hypothesis not safe for decoding or writing
~~~

Do not silently promote a strong prop observation into a universal character
rule. The current 1000/1001/1002/1004 linked layouts are confirmed for the
observed Bully PC families, while their cross-platform status remains open.

### Compare timing against known rates

Normalize candidate time fields against the chunk duration and test whether
keys land on plausible frame grids across multiple clips. This separated the
16-bit normalized time in 999/1003 from the 9-bit packed time in 1002/1004 and
caught the old 1004 byte-time hypothesis.

### Never use one animation to identify a skeleton

Static or action-only clips can hide an error. Use a library of clips, a bind
pose, a non-zero sample, and an independent rest-bridge check. A root or torso
curve can remain far from rest for an entire action; a calibrator that rejects
it must have a structurally justified recovery path or an honest partial state.

### Distinguish source IDs, imported IDs and runtime IDs

AGR `track_i`, NIF block indices, DFS order, HXD sequence indices and runtime
`NodeId`s are different namespaces. Name each conversion explicitly. The
player fix works because it records the synthetic scene-root/dummy offset
instead of pretending the source and imported indices are the same.

### Validate the deformed mesh, not only the skeleton

Bone endpoints and lengths can look correct while the mesh is twisted by a
wrong palette, inverse bind, partition map or curve assignment. Always compare
the static rest bridge and rendered skinned poses. For difficult cases, dump
posed node positions and a deterministic headless frame in addition to looking
at the GUI.

### Preserve unknown bytes and refuse unsafe writes

A parser can be useful before it is a serializer. Unknown AGR variants,
auxiliary 1002 records, compiler pointers and HXD padding should remain
diagnostic/raw data. Editing or round-tripping should wait until unknown
sections, sizes, padding and external dependencies can be preserved exactly.

### Make probes small, repeatable and adversarial

Prefer focused scripts that answer one question: record stride, time scale,
predecessor validity, HXD row alignment or model pairing. Add synthetic cases
for truncation, zero records, terminal identity keys, forward links, duplicate
times and absurd counts. A successful normal-file probe is not a safety test.

### Model partial coverage explicitly; all-or-nothing hides real data

The compound-catalog alignment originally required every AGR chunk to match a
catalog row, so `Area_GirlsDorm` (9 named rows for 15 chunks) showed no names
at all and looked like a parser failure. The corrected rule is a monotonic,
size-keyed alignment with an explicit skip cost for unmatched clips and rows:
covered clips take their catalog names and everything else keeps the
positional `clip_NN` fallback. Two lessons generalize beyond naming:

- A catalog that covers less than the file is normal, not an error; dropping
  every result for the whole file is strictly worse than a labeled partial
  result. When designing a matcher, make the unmatched case a first-class
  branch with an honest output, never a global bail-out.
- Before blaming a parse, inspect the unmatched content — and, before
  writing data off as corrupt, re-derive the row layout from raw bytes. The
  six uncovered `Area_GirlsDorm` chunks were legitimate one-frame held-pose
  stubs (§4.6), while `VAULT_BAR`/`MINISNOW_HITSHVL` were first mislabelled
  "stale/compiler-debris rows" and later proved to be valid rows dropped by
  a four-byte descriptor-offset error. The structural fix (pin the field
  start, validate floats/size/index, prefer the found position first) is
  the same discipline the AGR reader applies to records: bounded search,
  orthogonal evidence, no guessing — and a before/after per-row diff to
  prove the recovery moved nothing else.
- Instrument the resolver and census the corpus instead of reasoning from
  examples: the same audit that classified each unnamed clip
  (`stub`/`micro`/`motion`) reduced "are there more like this?" from
  speculation to a single-row list (§5 — one stub-only AGR after the
  fused-prefix rows were recovered) and gives a tripwire if a future
  catalog change starts hiding real motion.

### Keep external tools as oracles, not dependencies

Local Python parsers, independent packages and executable disassembly are
valuable for comparison, but each layout must be checked against raw bytes and
the app's own invariants. One local parser's compensated SkinPartition flag
read is a useful warning: an apparent agreement can come from two mistakes
canceling each other.

### Record provenance with every conclusion

Keep the source file, archive/loose origin, clip index, variant, model, probe
name and date beside a finding. This makes it possible to tell whether a result
came from a retail PC asset, a mission group, a synthetic fixture or a modified
community archive. It also prevents a mislabeled/incomplete archive from being
used as a platform specification.

## 10a. Lessons learned from GTA DFF/IFP format research (2026-09-16)

These are the lessons from implementing the GTA DFF skin/HAnim parser and
IFP animation playback — recorded because they differ from Bully's
Gamebryo formats and from common RenderWare assumptions.

### GTA ped DFFs are Y-up, not Z-up

Props, buildings, and Bully NIF models are Z-up (the character stands along
source Z). GTA SA ped DFFs are **Y-up** (the character stands along source
Y — the 3ds Max biped convention). Using `Zup.to_yup_matrix()` renders the
character lying face-up; `Xup` renders it on its side. Only `Yup.to_yup_matrix()`
(which is the identity matrix) stands it upright. The orientation must be
auto-detected per DFF — `has_hanim()` scans for HAnimPLG data and selects
Yup for skinned characters, Zup for everything else.

### SkinPLG matrices are NOT RenderWare-transformed

The SkinPLG bone matrices are stored as 16 f32 in a plain row-major layout
with translation in the bottom row (elements [12], [13], [14]). This means
feeding the stored rows directly as glam columns (`from_cols_array_2d`)
produces the correct column-vector affine matrix — no manual transpose
needed. The translation ends up in the last column, and the 3×3 linear part
is the rotation with rows-as-columns (which is the correct convention for
`from_cols_array_2d`).

### SkinPLG vertex data is two contiguous blocks, NOT interleaved

The per-vertex data is laid out as: a contiguous block of 4 × `vertices`
bytes (bone indices), followed by a contiguous block of 16 × `vertices`
bytes (f32 weights). DragonFF reads them as two separate arrays — NOT
interleaved per vertex. Interleaving them garbles every weight (the "weight"
floats read from index bytes produce denormals like 8.8e-50).

### SkinPLG header has a pad byte

The header is `3×u8` (num_bones, num_used_bones, max_weights_per_vertex)
followed by a **pad byte**. The pad must be skipped before reading the
used-bones array. Skipping it shifts the entire skin data by one byte,
producing garbage weights.

### IFP section headers are 8 bytes, not 12

IFP sections have a 2-field header: magic (4 bytes) + size (4 bytes). No
version word, no flags. This is fundamentally different from RenderWare's
12-byte header (kind + size + version). The alignment is also 4-byte
absolute (relative to the section start), applied after each section.

### ANP3 key sizes: type 3 = 10 bytes, type 4 = 16 bytes

The ANP3 compressed keyframe types map as: type 3 (CHILD) = 5 × i16 =
quat(4×i16/4096) + time(1×i16/30) = **10 bytes**; type 4 (ROOT) = 8 × i16 =
quat(4×i16/4096) + pos(3×i16/1024) + time(1×i16/30) = **16 bytes**. This
differs from rwfury's source order (which lists type 3 as 16 and type 4 as
10) — the mapping was empirically verified against `frame_data_size` for
all 294 animations in SA ped.ifp.

### ANPK object layout: bone_id at +24, key_count at +28, 12-byte tail

The ANIM section body starts with a 24-byte object name, then bone_id
(i32), then key_count (u32), then 12 bytes of zeros (flags/pointers that
don't affect decoding). The KR00/KRT0/KRTS section follows as a sibling
chunk (not nested inside ANIM).

### GTA ped DFFs and IFPs share bone names by design — no model matching needed

GTA has no "model → animation" mapping. All ped DFFs use the same biped
bone naming convention from 3ds Max, and all ped IFPs animate those same
bone names. Any ped IFP can play on any ped DFF via name-identity binding.
The game's association data (animgrp.dat) controls which animations are
available, not which models they work with. Played clips may legitimately
show a character's back because clips are authored relative to the game's
actor node (the Dummy placeholder the stream never animates), not the DFF's
bind pose.

### Python heredocs on Windows: always write to a file

Inline `python -c "..."` scripts containing heredoc syntax (`<<EOF`) hang
indefinitely on Windows Git Bash. Even simple `python -` with a heredoc
hangs. The reliable pattern is to write the script to a temp file
(`$TEMP/script.py`) and invoke it by path. Backticks in double-quoted
strings are also interpreted as command substitution by bash before the
script even sees them.

## 11. Recommended future work

In priority order:

1. [completed] Reduce variants 1000 and 1001 using the local executable descriptor table,
   targeted `C_Player`/rare-variant fixtures, bounded field hypotheses and
   pose validation. Do not infer them from 1002/1004.
2. [completed] Broaden the guarded AGR-to-NIF binding evidence across additional matching
   character/prop rigs. Keep the current fallback narrow and diagnostics-rich.
3. [partial] Preserve the 1002 auxiliary tail as opaque records (the reader now
   does this) while excluding it from playback; investigate its runtime inputs
   and purpose only if authoring, lossless round-trip or gameplay-accurate
   export requires it.
4. Build a CAT structural reader and evidence-labelled CAT → AGR relationship
   resolver; do not execute action nodes.
5. Add the deferred LIP structural inspector and `Speech.bin` association
   checks without claiming viseme decoding.
6. Add a read-only LUR bytecode inspector with strict Lua header/prototype
   bounds; never embed a Lua VM or execute archive scripts.
7. Add real cross-platform fixtures before making claims about Xbox 360, Wii,
   PS2, PSP, mobile or other Bully resource dialects.
8. Consider GPU skinning only after profiling large real models and keeping the
   CPU path as the numerical reference.
9. Optionally label single-frame held-pose stubs (§4.6) in the clip list (for
   example `clip_05 (held pose)`) once a second compound file confirms the
   duration-1/30 + two-keys-per-curve shape; recognize them by that shape,
   never by absence from a catalog, and never name them from a catalog row.

Editing/serialization remains a separate decision. Playback success is not
proof that an AGR can be safely rewritten.

## 12. Source map and historical notes

Primary raw evidence:

- [`../../bully-probe/FINDINGS.md`](../../bully-probe/FINDINGS.md) — probes,
  byte layouts, corpus tables and retired hypotheses;
- [`../../bully-probe/CHECKPOINT.md`](../../bully-probe/CHECKPOINT.md) — prior
  implementation handoff and reproduction commands;
- [`bully-agr-cat-lip-lur-roadmap.md`](bully-agr-cat-lip-lur-roadmap.md) —
  broader CAT/LIP/LUR plan;
- [`animation-viewer-infrastructure-plan.md`](animation-viewer-infrastructure-plan.md) —
  format-independent player architecture; and
- [`../TODO.md`](../TODO.md) — tracked phase checklist.

The local probe suite is outside the authoritative Rust repository:

~~~text
C:\Dev\IMGEditor-master\bully-probe\
~~~

Its generated JSON, extracted game data, executable dumps and exploratory
scripts must remain outside Git. This document records conclusions, not game
payloads.
