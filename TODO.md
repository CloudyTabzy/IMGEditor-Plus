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
- Initial observations from `observ4.nft` block 3 (43891 bytes) and block 7 (2907 bytes):
  ```
  Header (first ~72 bytes):
  +0:    pixel_format (u32) = 4  (RGB8? RGBA8? DXT?)
  +4-7:  ??? (0x00FFFFFF in both blocks — constant)
  +8-15: ??? (255, 256 — constant)
  +16-23: 1024, 1024? (0x0400 as u16 — seen in both)
  +24-27: 0
  +28-63: 3 × (width=19, height=5?) repeated — possibly mipmap or face descriptors
  +64:    sentinel (0xFFFFFFFF)
  +68:    count (9 in block 3, 7 in block 7 — different!)
  +72+:   array of (width?, height?, data_offset?) at variable offset
  ```
- Block 7: total 2907 bytes. At +80-87: (64, 64) — plausible texture size.
- Block 3: total 43891 bytes. At +80-87: (1, 1) — plausible smallest mip size.
- Pixel data starts somewhere after offset ~96 bytes; the remaining bytes
  are likely DXT-compressed (block 7: 2907-96=2811 px bytes fits 64x64 DXT).
- Key unknowns: pixel_format enum values for 20.3.0.9, exact mipmap descriptor
  layout, face count field location.

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
