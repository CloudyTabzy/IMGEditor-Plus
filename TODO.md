# IMGEditor-rs — Next Objectives

## 1. Reverse-engineer texture pixel data storage

The `.nft` (NIF texture catalog) files store **path metadata only** — source paths like
`Z:\Bully\Temp\Export\Textures\Scenes\iobserv\PO00_guts_d.tga` — but **not the actual pixel bytes**.

### Known facts

| Fact | Detail |
|------|--------|
| `World.img` has ~11980 entries | NIFs, NFTs, collisions, animations, and ~900 entries with **no file extension** |
| No `.tga`/`.dds` files exist on disk | The game directory has zero loose texture image files |
| NFT `NiSourceTexture.use_external == 0` | Implies embedded pixel data, but 0 bytes follow the header fields in practice |
| NFT has `NiPixelData` blocks | Our parser's field layout produces garbage values (`67108864px, 256bpp, 4294967040 faces`) — the real format differs from the nifxml schema |
| 902 unnamed IMG entries | Likely the texture data, stored under names that **don't match** the NFT's texture basenames |
| `.txd` files exist in `TXD\` dir | Only frontend/UI textures (7 files, ~6 MB total) — not world textures |

### Investigation approaches

**A. Cross-reference unnamed IMG entries with NFT source paths**
- For each unnamed IMG entry, read its raw bytes and compute a hash
- For each NFT, hash the known texture source paths and look for matches
- If a match is found, record the IMG entry name → texture mapping

**B. Study the NiPixelData format in the NFT NIFs**
- The current parser reads: `pixel_format`, `num_faces`, `num_mipmaps`, `bytes_per_pixel`, `mipmap_stored`, `num_pixels`, `num_frames`, `pixel_data`
- The garbage output suggests the actual 20.3.0.9 format has different field ordering or additional fields
- Investigate nifxml for version-specific `NiPixelData` schemas, or hex-dump actual NiPixelData blocks to reverse the format

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
