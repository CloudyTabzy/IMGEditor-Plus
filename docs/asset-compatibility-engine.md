# Asset Compatibility Engine — design & roadmap

Status: approved direction, pre-implementation design
Created: 2026-09-10
Owner: IMGEditor Plus
Companion docs: [renderware-format-mismatches.md](renderware-format-mismatches.md),
[renderware-gta-preview.md](renderware-gta-preview.md),
[bullyfury-adaptation-notes.md](bullyfury-adaptation-notes.md),
reference roadmap: `Docs/future_roadmap.md` Phase 25 (top-level, outside repo)

---

## 1. Vision

Today IMGEditor Plus reads, renders, and re-packs game assets. The next
goal is to make it the **gatekeeper**: before an archive is saved, the app
should know — and tell the user — whether every model and texture inside it
will actually load and render in the target game at runtime.

The problem it replaces is the modding workflow everyone knows:

> import → save → launch game → walk to the object → texture is pink /
> model is invisible / crash → guess → re-export → repeat.

That trial-and-error loop exists because raster/model compatibility rules
are folklore: they live in forum posts and tool defaults, not in any tool
the user owns. Our corpus forensics showed the knowledge is encodable — a
texture "works" when its raster profile matches what the target engine's
RenderWare layer consumes, and every one of those fields is parseable.

**One-sentence goal:** encode each supported game's asset dialect as a
machine-readable profile, validate every asset against the archive's
target engine, and convert imports through a loss ladder that is never
silent about what it costs.

---

## 2. Design principles

1. **Inform everything, block nothing.** Modded runtimes break the rules
   by definition (see the modded `gta3.img`: a GTA III archive full of SA
   assets that *works* in its runtime). Verdicts are always framed as
   "per the selected target engine" and always overridable.
2. **Never silent loss.** Any conversion that changes pixels (DXT
   compression, palette quantization, colorkey downgrades) requires
   explicit user confirmation with a preview. Lossless conversions
   (palette-native re-encode, format migration) can be automatic.
3. **Profiles are evidence-based and falsifiable.** Each rule cites its
   source: corpus measurement, RW/D3D documentation, or retail-archive
   verification. "Untested" is a legal verdict, not a gap to paper over.
4. **Format-stable by default.** Round-trips preserve the file's current
   profile unless the user explicitly asks for a migration or compression.
   No silent format drift — we just documented a whole corpus damaged by
   exactly that.
5. **Target engine is metadata, not inference.** The archive's identity
   (name, version) suggests a default target engine; the user can change
   it per archive. The modded `gta3.img` proves inference is unsound.

---

## 3. The knowledge model

### 3.1 RasterProfile (the atomic description)

Every texture's *actual* encoding, derived from the native header plus the
content cross-checks (see §5 of renderware-format-mismatches.md):

| Field | Source | Trust |
|---|---|---|
| platform id (8 = D3D8, 9 = D3D9) | native header | high |
| RW raster format code (nibble) | native header | low — tools leave it stale |
| D3D format word / FourCC | native header | authoritative on D3D9 |
| logical format (1555/565/4444/888/8888/LUM8/A8L8/PAL4/PAL8/DXTn) | derived | resolved through the cross-check chain |
| storage width (bytes/px) | depth byte + data length | high |
| palette presence/size | extension bits | medium |
| alpha usage | header flags + pixel scan | pixel scan is ground truth |
| mip chain | mip count + extension flag | high |
| palette-reconstructible | unique-color count ≤ 256 | pixel-scan (corpus-proven) |

### 3.2 GameProfile (the rulebook)

A static, auditable table per supported game/runtime. Each raster class
gets a verdict:

| Verdict | Meaning | App behavior |
|---|---|---|
| **Native** | engine-authored format | ✓ badge, no action |
| **Supported** | loads, but wasteful or unusual (e.g. 32-bit where the game ships 8-bit) | ✓ badge + note |
| **Convertible** | loads after a lossless transform | ⚠ badge + one-click convert |
| **Lossy-convertible** | needs DXT/quantize/colorkey | ⚠ badge + gated dialog with preview |
| **Unsupported** | engine cannot consume it | ✗ badge + explanation |
| **Untested** | no evidence either way | ? badge + what to test |

Verdicts always name the *cost*: "DXT1: 4× smaller, gradients band";
"palette → 888: size ×4, pixels identical"; "1555: alpha collapses to
1-bit colorkey".

### 3.3 Draft knowledge tables (evidence today)

**GTA San Andreas PC** (RW 3.6, D3D9, platform 9) — corpus: retail SA
archive needed for final confirmation; current evidence from the modded
corpus + RW knowledge:

| Class | Verdict | Evidence |
|---|---|---|
| DXT1 / DXT3 (+ mips) | Native | corpus: 1,508 + 413 + 378 files intact |
| PAL8 / PAL4 | Native | SA ped skins famously paletted; `torso8bit` fossil; retail SA verification pending |
| 888 logical (stored X8R8G8B8 32bpp) | Supported | corpus: the 888-class storage convention itself |
| 8888 | Supported | corpus: 33 alpha tiles |
| 1555 | Native (small props) | corpus: small 64² alpha props — nibble-vs-fourcc caveat applies |

**GTA III / Vice City PC** (RW 3.3–3.4, D3D8, platform 8) — corpus:
**retail III/VC archive required** (the local corpus is a modded SA-dialect
archive, deliberately not trusted for III parity):

| Class | Verdict | Evidence |
|---|---|---|
| DXT1 (via platform-properties byte) | Native | D3D8-era RW mapping (decoder already implements it) |
| 888 logical stored 24bpp (3 B/px) | Native (probable) | D3D8 has a real 24-bit format; needs retail confirmation |
| PAL8 / PAL4 | **Untested** — the biggest open question | PS2-era conversions suggest yes; retail verification required |
| X8R8G8B8 D3D9 rasters | Unsupported (as-is) | D3D8 has no such format; lossless rewrite to 24-bit or 8888 |
| SA RW 3.6 stream sections | Convertible (version rewrite) | RW version field lives in every section header |

**Bully SE** (Gamebryo NIF/NFT, not RenderWare) — existing pipeline is the
authoritative reader (see bullyfury notes); a Bully profile enumerates
`NiPixelData` formats (DXT1/DXT3/DXT5 + raw), endian variants, and the
NFT catalog resolution rules. COL3's Bully variant is documented as
Bully-specific.

**Cross-game conversion is almost always possible — the question is cost.**
The validator's vocabulary reflects that instead of a binary works/broken.

### 3.4 DFF/model profiles (later phase)

The same model extends to models: RW stream version, geometry flags,
prelight colors, UV animation dictionaries (`0x2B`), vertex formats. This
overlaps the existing roadmap's "Phase 25 — DFF/RW version conversion".
Sequence the texture side first (smaller, already 90% implemented); the
DFF side reuses the architecture.

---

## 4. Architecture

New modules (proposed):

```
src/compat/
  mod.rs        — GameProfile registry, TargetEngine selection
  raster.rs     — RasterProfile extraction from the existing native parser
  verdict.rs    — classify(RasterProfile, GameProfile) -> Verdict { cost }
  convert.rs    — the loss ladder: plan -> preview -> confirm -> encode
src/ui/compat.rs — badges, target-engine selector, conversion dialogs
```

Key point: **the parser already produces everything the validator needs.**
`parse_txd` → `NativeTexture` has platform, raster format, D3D word, depth,
mips; the decoder already applies the cross-check chain; the unique-color
scan is one pass over decoded pixels. No new format code — only judgment
code on top of formats we already read.

Integration points (all existing surfaces):

| Surface | Change |
|---|---|
| Texture tab | verdict badge + cost note next to the format name |
| 3D view load path | warn when a model's textures carry non-native profiles |
| Import files | validate on import; conversion dialog keyed to target engine |
| Save | final pass: "3 textures will not load in GTA III — convert?" |
| Inspector summary | palette-reconstructible flag (N colors) |

---

## 5. The conversion ladder (phase B)

Ordered, never silent:

1. **Palette-native** (opt-in): if palette-reconstructible and the target
   profile supports palettes → reconstruct palette, remap indices. 256²
   skin: 64 KB instead of 262 KB. Requires the target-engine palette
   question answered first.
2. **Format-stable** (default): re-encode to the file's *current* profile.
   Perfect round-trip, zero risk.
3. **Lossless migration**: e.g. D3D9 → D3D8 cross-platform rewrite with a
   real 24-bit target for 888 content; version-field downgrade.
4. **Lossy, gated**: DXT compression (offer variants, preview banding),
   palette quantization (preview the palette, list dropped colors),
   RGBA → 1555 colorkey.

Every step produces a plan the user can inspect ("rewrite 355 textures to
X8R8G8B8-consistent 8888: +38 MB") before anything is written — the save
path is already verbatim-copy, so conversion must be an explicit, separate
action, never a side effect.

---

## 6. UX surfaces

- **Badges** in the Texture tab and 3D view: ✓ / ⚠ / ✗ / ? with a tooltip
  carrying the cost sentence.
- **Per-archive target engine** in the archive properties/toolbar: default
  inferred from archive identity, user-changeable, persisted in settings.
- **Import dialog**: "This PNG has 412 colors; target GTA III SA-era
  palettes cap at 256. [Quantize + preview] [Store as 8888 (+size)]".
- **Pre-save report**: the save flow already streams; add a validation
  summary pass with counts per verdict and a "normalize" option.
- **Normalize archive** (phase C): rewrite self-inconsistent files to a
  coherent profile (e.g., the dwayne-class 888 files → consistent 8888 or
  true 24-bit) for the benefit of *other* tools, not just ours.

---

## 7. Phasing — and where to start

### Phase 0 — Profile knowledge tables + verification corpus (**START HERE**)

A doc-first phase. Deliverables:

1. `src/compat` profile tables as *data* (rust constants/tables), drafted
   from §3.3, each entry tagged `evidence: corpus | docs | retail | untested`.
2. **Reference corpus acquisition**: retail (or user-owned) GTA III, VC,
   SA, Bully archives to close the untested cells. A small manifest +
   scanner (the Python forensics from this session, ported to a Rust
   `--scan-corpus` diagnostic or kept as tooling scripts) that profiles an
   archive and diffs it against the table.
3. The verification matrix (below) executed against the corpora; every
   `untested` cell resolved or documented as such.

Why first: every later decision hangs on these tables being right, and the
doc+scanner phase produces value (a corpus auditor) even before any UI
exists. Estimated effort: a few days, mostly verification.

### Phase A — Validator read-side (badges)

`classify()` over existing parses; badges in Texture tab / 3D view /
import; unique-color flag in the inspector. No writing. Est: 1 week.

### Phase B — Converter + loss ladder

Lands together with the texture-replace feature (same machinery). Import
PNG/DDS → plan → preview → encode. Palette-native tier only after Phase 0
answers the palette questions. Est: 2–3 weeks including tests.

### Phase C — Target-engine metadata + normalize pass + corpus auditor UI

Per-archive target setting; pre-save validation report; normalize/repair
pass for self-inconsistent rasters; corpus auditor as a menu tool. Est: 1–2
weeks.

### Phase D — Model/DFF profiles

Same architecture over DFF streams (RW version, geometry, prelight, UV
anims); supersedes/absorbs roadmap Phase 25. Est: 2–3 weeks.

---

## 8. Verification matrix (Phase 0 checklist)

| # | Question | How to verify | Blocks |
|---|---|---|---|
| 1 | Does GTA III PC (D3D8) accept PAL8/PAL4 rasters? | Find PAL rasters in a retail III/VC corpus; if absent, engine-source lore + safe default = convert to uncompressed | palette-native tier for III targets |
| 2 | What D3D format word does SA use for paletted D3D9 rasters? | Retail SA corpus: inspect PAL8 natives' format word | writing PAL8 for SA targets |
| 3 | Mip policy per game (required? generated?) | Corpus mip-count distribution per class | normalize pass, DXT writes |
| 4 | DXT3 vs DXT1 acceptance in III/VC | Corpus: III ships DXT3? (VC/III era known to use DXT1/3) | conversion targets |
| 5 | 888 on III: true 24-bit or also 32-bit-padded? | Retail III corpus bytes/px measurement | cross-platform rewrites |
| 6 | RW section version tolerance (3.6 sections in a 3.4 game?) | The modded corpus proves *some* runtimes accept it; test with the mod's target runtime | target-engine metadata design |
| 7 | Bully NFT profile | Existing tests + World.img corpus | Bully verdicts |

---

## 9. Risks & non-goals

- **Corpus ≠ runtime truth.** A profile table derived from modded data
  describes what tools produce, not what engines accept. Phase 0 exists to
  keep those separate. The local `Gta_3_img` corpus is *dialect evidence*
  (docs/renderware-gta-preview.md already flags it as possibly mislabeled).
- **No PS2/Xbox/mobile targets initially.** PC first, consistent with the
  app; the profile model leaves room for platform variants later.
- **No automatic re-encoding during save.** Save stays verbatim; conversion
  is always an explicit user action with a plan preview.
- **Do not let badges regress performance.** Verdicts compute once per
  texture (cacheable alongside the existing preview caches), never per
  frame.

## 10. References

- `docs/renderware-format-mismatches.md` — the X8R8G8B8 case study, the
  cross-check discipline, corpus provenance forensics (unique-color
  method, 257–259 fingerprint, DXT-passthrough vs palette-reencode split).
- `docs/renderware-gta-preview.md` — DFF/TXD coverage, corpus caveat,
  decoder inventory.
- `docs/bullyfury-adaptation-notes.md` — Bully NIF/NFT dialect.
- `docs/gta-img-reference-audit.md` — IMG container behaviors (v1/v2/Xbox).
- `Docs/future_roadmap.md` Phase 25 (top-level) — prior DFF/RW conversion
  plan, absorbed by Phase D here.
- Session forensics scripts (temp): `txd_classify.py`, `palette_origin.py`
  — the methodology to port into the Phase 0 scanner.
