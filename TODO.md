# IMGEditor-rs — Next Objectives

## 1. Reverse-engineer texture pixel data storage

The `.nft` (NIF texture catalog) files store **path metadata only** — source paths like
`Z:\Bully\Temp\Export\Textures\Scenes\iobserv\PO00_guts_d.tga` — but **not the actual pixel bytes**.

### Known facts

| Fact | Detail |
|------|--------|
| `World.img` has 11980 entries | 5724 NIFs, 4469 NFTs, 550 `.agr`, 493 `.lip`, 488 `.col`, 119 `.cat`, 85 `.ipb`, 52 `.lur`. **Zero `.tga`/`.dds`** entries |
| NFT `NiSourceTexture.use_external == 0` | Implies embedded pixel data, but 0 bytes follow the header fields in practice |
| NFT has `NiPixelData` blocks | e.g. **43891 bytes** of data for `observ4.nft` — the pixels ARE there but the header format is unknown |
| NiPixelData header (attempted) | `pixel_format=4` at offset 0 (plausible), but `num_faces=0xFFFFFF00` garbage at offset 4 |
| Plausible mipmap entries at offset+28 | `19×5`, repeated 3 times with `data_offset=0` — suggests a variable-length mipmap table before pixel data |
| Pixel data starts somewhere after offset 64+ | Full block is 43891 bytes, so data section is ~43800 bytes |
| No `.tga`/`.dds` on disk at all | Confirmed: neither as loose files nor as named IMG entries. Pixels live **inside NFT NIFs** |
| `.txd` files in `TXD\` dir | Only frontend/UI textures (7 files, ~6 MB total) — not world textures |

### Investigation approaches

**A. Reverse NiPixelData header for 20.3.0.9**
- Dump raw bytes of a known NiPixelData block and reverse the field layout.
- Initial observations from `observ4.nft` block 3 (43891 bytes):
  ```
  +0:  pixel_format = 4 (RGBA8 plausible)
  +4:  7 bytes of unknown structure
  +11: ???
  +16: width? = 4
  +20: height? = 4
  +28: 19, 5, 0    ← mip entry 0: w=19, h=5, data_offset=0?
  +40: 19, 5, 0    ← mip entry 1
  +52: 19, 5, -1   ← mip entry 2 (sentinel?)
  +64: face/misc fields, then raw pixel bytes
  ```
- The exact header structure needs to be determined by comparing multiple NiPixelData blocks with different texture dimensions.

**B. Cross-reference unnamed IMG entries with NFT source paths**
- For each unnamed IMG entry, read its raw bytes and compute a hash
- For each NFT, hash the known texture source paths and look for matches
- If a match is found, record the IMG entry name → texture mapping

**C. Scan the IMG directory for entries containing TGA/DDS magic bytes**
- Even without matching names, the first few bytes of each entry can reveal the format (TGA starts with `0x00 0x00 0x02`, DDS with `0x44 0x44 0x53 0x20`)
- Build a `{magic → entry_name}` map to identify which entries hold texture data

**D. Search for the NFT source paths as binary strings inside World.img**
- The full paths like `Z:\Bully\Temp\Export\Textures\Scenes\iobserv\PO00_guts_d.tga` may appear as ASCII strings in the IMG data near their corresponding pixel data

## 2. Intermediate format options (while texture extraction is incomplete)

- **Checkerboard/placeholder texture**: when pixel data isn't available, generate a coloured
  checkerboard TGA so the user can at least see UV mapping in F3D
- **Vertex colour fallback**: if the NIF has vertex colours, write them to the PLY and let
  F3D display them (PLY supports per-vertex colours with `property uchar red` etc.)

## 3. Quality-of-life improvements

- Cache parsed NFT catalogs (the same NFT serves many NIFs)
- Add a CLI or GUI option to specify the game root path (instead of deriving from archive path)
- Clear old temp files on startup (`%TEMP%\IMGEditor\preview\`)
