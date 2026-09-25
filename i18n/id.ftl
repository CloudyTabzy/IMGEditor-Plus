### IMG Editor Plus - Bahasa Indonesia.
###
### Draft translation; pending review by a native speaker.
### Keep message ids and { $variables } exactly as in en.ftl.

## Menu bar: root menus

menu-file = Berkas
menu-recent = Terbaru
menu-edit = Edit
menu-selection = Pilihan
menu-view = Tampilan
menu-themes = Tema
menu-help = Bantuan
menu-language = Bahasa

## File menu

menu-file-new = Baru ({ $shortcut })
menu-file-open = Buka… ({ $shortcut })
menu-file-save = Simpan ({ $shortcut })
menu-file-save-as = Simpan sebagai… ({ $shortcut })
menu-file-pack = Kemas arsip
menu-file-set-game-folder = Atur folder game…
menu-file-reset-game-folder = Atur ulang folder game
menu-file-close-tab = Tutup tab ({ $shortcut })
menu-file-sort-by = Urutkan berdasarkan…

## Recent menu

menu-recent-empty = Tidak ada berkas terbaru

## Edit menu

menu-edit-import = Impor ({ $shortcut })
menu-edit-import-folder = Impor folder
menu-edit-export-all = Ekspor semua ({ $shortcut })
menu-edit-export-selected = Ekspor pilihan ({ $shortcut })
menu-edit-export-list = Ekspor sebagai daftar ({ $shortcut })
menu-edit-compare-list = Bandingkan dengan daftar ({ $shortcut })
menu-edit-load-agr = Muat berkas animasi .agr…

## Selection menu

menu-selection-all = Pilih semua ({ $shortcut })
menu-selection-invert = Balikkan pilihan ({ $shortcut })
menu-selection-clear = Hapus pilihan ({ $shortcut })
menu-selection-delete = Hapus yang dipilih ({ $shortcut })

## View menu (toggles; the on/off marker is added by the code)

menu-view-navigation-gizmo = Gizmo navigasi
menu-view-search-bar = Bilah pencarian
menu-view-search-selection-context = Konteks pilihan dalam pencarian
menu-view-literal-file-types = Tipe berkas literal
menu-view-highlight-validator-rows = Sorot baris validator
menu-view-context-accumulates = Klik kanan menambah ke pilihan
menu-view-autoscroll-momentum = Momentum gulir otomatis
menu-view-motion-effects = Efek gerakan
menu-view-selection-pulse = Denyut pilihan
menu-view-click-ripples = Riak klik
menu-view-icon-micro-motion = Gerakan mikro ikon
menu-view-animation-demo = Demo animasi (sintetis)
menu-view-explorer-association = Buka .img/.dir dari File Explorer

## Themes menu (named themes such as Catppuccin Mocha keep their names)

theme-dark = Gelap
theme-light = Terang

## Help menu

menu-help-check-updates = Periksa pembaruan ({ $shortcut })
menu-help-repository = Kunjungi repositori
menu-help-about = Tentang

## Language menu (each language is listed under its own name)

# $language is the detected system language's own name, e.g. "Español".
menu-language-system = Bahasa sistem ({ $language })
menu-language-pseudo = Pseudolokal (uji tata letak)

## Entry context menu (right-click on an archive entry)

# Shown under the entry name when the right-click extends a selection.
context-more-selected = +{ $count } lainnya dipilih
context-play-animation = Putar animasi
context-open-3d = Buka di penampil 3D
context-open-external = Buka di penampil eksternal
context-view-textures = Lihat tekstur
context-export-companion-textures = Ekspor tekstur NFT pendamping
context-export-embedded-textures = Ekspor tekstur tersemat
context-export = Ekspor
context-rename = Ganti nama
context-copy-name = Salin nama
context-delete = Hapus

## Shared dialog buttons and labels

button-close = Tutup
button-cancel = Batal
button-save = Simpan
button-discard = Buang
button-apply = Terapkan
button-reset = Atur ulang
button-convert = Konversi
button-planning = Menyusun rencana…
field-format = Format:
field-name = Nama:
# Stands in for a game name in sentences like "checked against { $target }".
target-unset = target
target-unset-selected = target yang dipilih

## About dialog

dialog-about-title = Tentang
about-body =
    IMG Editor Plus v{ $version }

    Editor desktop Rust murni untuk arsip IMG GTA.

    Dibuat oleh CloudyTabzy & agen
    Berdasarkan IMG Editor asli karya Grinch_
    (https://github.com/user-grinch/IMGEditor)

    Format yang didukung:
    - GTA III
    - GTA Vice City
    - GTA San Andreas
    - Bully Scholarship Edition
about-visit-repository = Kunjungi repositori

## Welcome dialog

dialog-welcome-title = Selamat datang
welcome-heading = Selamat datang di { $app } v{ $version }
welcome-tagline = Editor arsip GTA untuk III, VC, San Andreas, dan Bully SE.
welcome-dont-show = Jangan tampilkan pesan ini lagi
welcome-disable-updates = Nonaktifkan pemeriksaan pembaruan
welcome-get-started = Mulai

## Unsupported format dialog

dialog-unsupported-title = Format tidak didukung
unsupported-body = Format IMG tidak didukung.
unsupported-path = Jalur: { $path }
unsupported-supported = Format yang didukung: GTA III, Vice City, San Andreas, Bully SE.

## Texture converter dialogs (replace texture, image as TXD, bulk convert)

format-option-native = { $format } (asli)
format-option-opt-in = { $format } (opsional)
dxt-high-quality = DXT kualitas tinggi (cluster fit, lebih lambat)
preview-full-quality = Lihat pada kualitas penuh
preview-full-quality-title = { $label } — kualitas penuh
preview-navigation-hint = Gulir untuk memperbesar · seret untuk menggeser · Esc untuk menutup
preview-current = Saat ini
preview-after = Sesudah (terkode)
dialog-target = Target: { $target }
dialog-target-archive = Target: { $target } · { $archive }
dialog-replace-title = Ganti tekstur
replace-summary = Mengganti “{ $texture }” — sumber: { $source } ({ $width }x{ $height })
replace-override-note = Disimpan di memori sebagai pengganti; berkas arsip berubah saat Anda menyimpan.
replace-confirm = Ganti tekstur
dialog-new-txd-title = Impor gambar sebagai TXD
new-txd-summary = TXD baru dari { $source } ({ $width }x{ $height })
new-txd-name-placeholder = nama tekstur
new-txd-confirm = Tambahkan ke arsip
dialog-bulk-title = Konversi ke dialek target
bulk-entry = { $entry } — { $count } tekstur -> { $formats }
bulk-more-entries = … dan { $count } entri lainnya
bulk-summary = { $textures } tekstur dalam { $entries } entri dari { $source } akan dikodekan ulang untuk target.
bulk-skipped = { $skipped } sudah asli (dilewati), { $failed } tidak terbaca (dilewati).
bulk-ignored = { $count } dari entri yang dipilih bukan wadah TXD dan akan dibiarkan apa adanya.
bulk-verbatim-note = Tekstur dan nama yang tidak disentuh tetap apa adanya; arsip berubah saat Anda menyimpan.

## Compare with list dialog

dialog-compare-title = Bandingkan dengan daftar
compare-manifest = Manifes: { $path }
compare-archive = Arsip: { $archive } · { $count } entri arsip
compare-stats = { $names } nama manifes ({ $unique } unik) · { $matched } cocok · { $missing } hilang ({ $missing-unique } unik)
compare-case-sensitive = Peka huruf besar/kecil
compare-show-archive-only = Tampilkan entri hanya-arsip
compare-duplicate-lines = { $count } baris manifes duplikat
compare-blank-lines = { $count } baris kosong diabaikan
compare-missing-heading = Hilang dari arsip ({ $count })
compare-no-missing = Tidak ada entri yang hilang.
compare-more-missing = … dan { $count } nama hilang lainnya
compare-archive-only-heading = Entri hanya-arsip ({ $count })
compare-no-archive-only = Tidak ada entri hanya-arsip.
compare-more-archive-only = … dan { $count } nama hanya-arsip lainnya
compare-copy-missing = Salin nama yang hilang
compare-running-heading = Membandingkan daftar entri
compare-running-body = Membaca { $manifest } dan membandingkannya dengan { $archive }…

## Save check dialog

dialog-save-check-title = Pemeriksaan penyimpanan
save-check-summary = { $textures } tekstur, { $entries } entri — diperiksa terhadap { $target }.
save-check-counts = { $fine } asli/didukung · { $convertible } dapat dikonversi (tanpa kehilangan) · { $incompatible } tidak kompatibel · { $unknown } tidak diketahui
# $note is a technical detail and stays in English.
save-check-container = Wadah: { $note }
save-check-anomaly = { $code }: { $count } (mis. { $example })
save-check-broken-headers = Header rusak (dapat diperbaiki tanpa pengodean ulang):
save-check-warnings = { $count } anomali tingkat peringatan (dilaporkan, tidak memblokir).
save-check-repair = Perbaiki header DXT yang tidak konsisten sebelum menyimpan ({ $count } laporan, tanpa kehilangan)
save-check-repair-note = Perbaikan memperbaiki bidang header DXT di tempat; tidak ada piksel yang dikodekan ulang.
save-check-verbatim-note = Penyimpanan menulis setiap entri apa adanya; tidak ada tekstur yang dikodekan ulang atau dikonversi.
save-check-fix-and-save = Perbaiki & simpan
save-check-save-anyway = Tetap simpan

## Unsaved changes dialog

dialog-unsaved-title = Perubahan belum disimpan
unsaved-archive = “{ $archive }” memiliki perubahan yang belum disimpan.
unsaved-archive-note = Menutup tanpa menyimpan akan membuang perubahan; berkas arsip di disk tidak tersentuh.
unsaved-window = { $count } arsip memiliki perubahan yang belum disimpan: { $archives }
unsaved-window-note = Keluar sekarang akan membuang perubahan; berkas di disk tidak tersentuh.
unsaved-discard-and-quit = Buang perubahan dan keluar

## Import check dialog

dialog-import-check-title = Pemeriksaan impor
import-check-offender = { $name }: { $verdict }
import-check-offender-note = { $name }: { $verdict } — { $note }
import-check-file = { $count } tekstur: { $detail }
import-check-more = …dan { $count } berkas lain yang ditandai.
import-check-summary = Keputusan diperlukan untuk { $target }: { $flagged } dari { $total } berkas — { $incompatible } tekstur tidak kompatibel, { $unknown } tidak diketahui.
import-check-note = Impor tetap apa adanya — format hanya penting jika game harus memuat tekstur ini.
import-check-import-anyway = Tetap impor
import-check-cancel = Batalkan impor

## Import folder dialog

dialog-folder-import-title = Impor folder
folder-import-path = Folder: { $path }
folder-import-summary = { $count } berkas biasa • { $size }
folder-import-top-level = Hanya berkas yang langsung berada di folder ini yang disertakan; subfolder tidak dipindai.
folder-import-no-duplicates = Tidak ada nama duplikat yang terdeteksi.
folder-import-duplicates = { $count } nama duplikat terdeteksi. Pilih cara menanganinya.
folder-import-skipped = { $count } item tidak dapat diperiksa dan akan dilewati.
folder-import-files = Impor berkas
folder-import-skip-duplicates = Impor (lewati duplikat)
folder-import-replace-duplicates = Ganti duplikat

## Update check dialog

dialog-update-title = Pemeriksaan pembaruan
update-available = Pembaruan tersedia: { $version }
update-latest = Anda menggunakan versi terbaru.
# $error is a technical detail and stays in English.
update-failed = Pemeriksaan pembaruan gagal: { $error }
update-dont-show = Jangan tampilkan pesan ini lagi
update-open-releases = Buka rilis

## Validate textures dialog

dialog-validator-title = Validasi tekstur
validator-intro = Pilih game yang menjadi target arsip ini. Validator menandai setiap tekstur di luar format yang diterima mesin retail tersebut dan mencantumkan yang tidak diketahui yang mungkin dapat dimuat tetapi bukan format asli game.
validator-last-run = Proses terakhir: { $counts }
validator-no-textures = tidak ada tekstur
validator-current-target = target saat ini
validator-validate-for = Validasi untuk { $game }
validator-native-heading = Asli (terverifikasi retail):
validator-unknown-heading = Tidak diketahui / bukan asli game:
validator-hint-likely = Konten tampak seperti { $game }
validator-hint-possible = Konten mungkin { $game }
validator-hint-current = target saat ini: { $game }
validator-use-as-target = Gunakan { $game } sebagai target
validator-already-target = (sudah menjadi target)
validator-highlight-rows = Sorot baris
legend-native = asli
legend-native-description = Dibuat oleh mesin ini — tidak perlu tindakan.
legend-supported = didukung / dapat dikonversi
legend-supported-description = Dapat dimuat, tetapi bukan dialek data game; penulisan ulang tanpa kehilangan mungkin ditawarkan.
legend-lossy = konversi dengan kehilangan
legend-lossy-description = Hanya dapat digunakan setelah konversi yang mengubah piksel (kompresi atau kuantisasi).
legend-unknown = tidak diketahui
legend-unknown-description = Tidak ada bukti ke arah mana pun — belum diketahui adanya ketidakcocokan. Gunakan dengan hati-hati.
legend-incompatible = tidak kompatibel
legend-incompatible-description = Mesin yang dipilih tidak dapat memakai format ini.

## Sort by dialog

dialog-sort-title = Urutkan berdasarkan — { $archive }
dialog-sort-title-no-archive = Urutkan berdasarkan — (tidak ada arsip terbuka)
sort-empty = Belum ada kriteria. Tambahkan kriteria di bawah untuk mulai mengurutkan.
sort-intro = Tetapkan aturan prioritas, lalu terapkan ke arsip saat ini.
sort-select-key = Pilih kriteria…
sort-add-key = + Tambah kriteria
sort-add-key-max = + Tambah kriteria (batas tercapai)
sort-keys-active = { $active } dari { $total } kriteria aktif
sort-preview-heading = Pratinjau langsung (10 entri pertama)
sort-preview-empty = (tidak ada entri di arsip saat ini)
sort-apply-preset = Terapkan preset…
sort-ascending-short = Naik ▲
sort-descending-short = Turun ▼
sort-key-name = Nama
sort-key-extension = Ekstensi
sort-key-type = Tipe
sort-key-size = Ukuran
sort-key-offset = Offset
sort-key-ide-file = Berkas IDE
sort-key-col-file = Berkas COL
sort-preset-name-az = Nama (A→Z)
sort-preset-name-za = Nama (Z→A)
sort-preset-type-then-name = Tipe, lalu nama
sort-preset-size-desc = Ukuran (besar → kecil)
sort-preset-offset-asc = Offset (rendah → tinggi)

## Windows file dialogs (titles and file-type filters)

file-dialog-open-archive = Buka arsip IMG
file-dialog-filter-img = Arsip IMG
file-dialog-compare = Bandingkan dengan daftar entri
file-dialog-filter-entry-list = Daftar entri IMG
file-dialog-open-agr = Buka grup animasi Bully
file-dialog-filter-agr = Grup animasi Bully
file-dialog-import-files = Impor berkas
file-dialog-filter-importable = Berkas yang dapat diimpor
file-dialog-import-folder = Pilih folder untuk diimpor
file-dialog-game-folder = Pilih folder game
file-dialog-choose-image = Pilih gambar
file-dialog-filter-images = Gambar
file-dialog-save-archive = Simpan arsip IMG
file-dialog-export-list = Ekspor daftar entri
file-dialog-export-folder = Pilih folder ekspor

## Toasts: archives and selection

toast-no-archive-selected = Tidak ada arsip yang dipilih.
toast-archive-closed = Arsip tidak lagi terbuka.
toast-target-archive-closed = Arsip target tidak lagi terbuka.
toast-target-archive-deselected = Arsip target tidak lagi dipilih.
toast-validated-archive-closed = Arsip yang divalidasi telah ditutup.
toast-entry-unavailable = Entri yang dipilih tidak lagi tersedia.
toast-task-running = Tugas lain masih berjalan.
toast-operation-running = Operasi arsip sudah berjalan.
toast-open-archive-to-validate = Buka arsip terlebih dahulu untuk memvalidasinya.
toast-open-archive-to-import = Buka arsip terlebih dahulu untuk mengimpor ke dalamnya.
toast-open-archive-to-import-folder = Buka arsip terlebih dahulu untuk mengimpor folder.
toast-open-img-for-model = Buka arsip IMG terlebih dahulu; model berasal darinya.
toast-already-open = Sudah terbuka: { $path }
# $error is a technical detail and stays in English.
toast-open-failed = Gagal membuka arsip: { $error }
toast-file-gone = Berkas tidak ada lagi: { $path }
toast-file-not-found = Berkas tidak ditemukan: { $path }
toast-compare-empty-archive = Arsip yang dipilih tidak memiliki entri untuk dibandingkan.

## Toasts: saving and packing

toast-archive-saved = Arsip disimpan.
toast-save-cancelled = Penyimpanan dibatalkan.
toast-save-failed = Penyimpanan gagal: { $error }
toast-save-before-pack = Simpan arsip sebelum mengemasnya.
toast-packed = Arsip dikemas — { $reclaimed } diklaim kembali ({ $size } di disk).
toast-packed-nothing = Arsip dikemas — tidak ada ruang yang diklaim kembali ({ $size } di disk).
toast-pack-failed = Pengemasan gagal: { $error }
toast-headers-repaired = { $count } header tekstur diperbaiki; menyimpan.

## Toasts: game folder

toast-game-folder-set = Folder game untuk { $archive }: { $path }
toast-game-folder-automatic = Folder game untuk { $archive }: { $path } (otomatis)
toast-game-folder-none = { $archive } tidak memiliki folder game.
toast-game-folder-needs-save = Simpan arsip terlebih dahulu; folder game disimpan per berkas arsip.

## Toasts: 3D viewer and animation

toast-select-model = Pilih entri NIF, DFF, atau COL terlebih dahulu.
toast-viewer-unsupported = Penampil 3D bawaan mendukung .nif, .dff, dan .col ({ $entry }).
toast-select-ifp = Pilih entri .ifp terlebih dahulu.
toast-no-model-for-agr = Tidak ada model .nif yang cocok untuk { $animation } di arsip ini.
toast-no-model-for-agr-open = Tidak ada model .nif yang cocok untuk { $animation } di arsip yang terbuka.
toast-loading-animation = Memuat { $animation } pada { $model }…
toast-loading-animation-hxd = Memuat { $animation } pada { $model }… (katalog HXD ditemukan)
toast-playback-stalled = Pemutaran dijeda setelah macet lama.
toast-demo-loaded = Demo animasi sintetis dimuat (tanpa data game).
toast-demo-closed = Demo animasi ditutup.
toast-animation-ready = Animasi siap: { $summary }
toast-animation-failed = Gagal memuat animasi: { $error }
toast-3d-load-failed = Gagal memuat 3D: { $error }
anim-summary-ifp = { $animation } pada { $model } ({ $clips } klip, GTA IFP)
anim-summary-agr = { $animation } pada { $model } ({ $clips } klip)
anim-summary-agr-named = { $animation } pada { $model } ({ $clips } klip, bernama HXD)

## Toasts: textures and conversion

toast-select-texture = Pilih entri tekstur terlebih dahulu.
toast-no-target = Belum ada target untuk arsip ini. Pilih gamenya di Validasi tekstur.
toast-target-error = { $error } Pilih game untuk arsip ini di Validasi tekstur.
toast-replace-busy = Penggantian sedang disiapkan.
toast-preparing-replacement = Menyiapkan penggantian…
toast-replace-failed = Penggantian gagal: { $error }
toast-import-busy = Impor sedang disiapkan.
toast-preparing-import = Menyiapkan impor…
toast-txd-name-required = Beri nama untuk TXD baru.
toast-entry-exists = Entri bernama “{ $name }” sudah ada.
toast-entry-added = “{ $name }” ditambahkan — simpan arsip untuk menulisnya.
toast-set-target-first = Tetapkan target game terlebih dahulu (Validasi tekstur).
toast-select-entries-to-convert = Pilih entri yang akan dikonversi terlebih dahulu.
toast-archive-changed-planning = Arsip berubah saat konversi direncanakan; coba lagi.
toast-convert-only-txd = Hanya entri TXD yang dapat dikonversi — tidak ada entri yang dipilih yang merupakan wadah tekstur TXD.
toast-convert-all-native = Semua tekstur yang dipilih sudah asli untuk target.
toast-archive-changed-after-plan = Arsip berubah setelah konversi ini direncanakan; coba lagi.
toast-archive-changed-during = Arsip berubah selama konversi; hasil usang dibuang.
toast-converted = { $count } entri dikonversi — simpan arsip untuk menulisnya.
toast-conversion-failed = Konversi gagal: { $error }
toast-no-decoded-textures = Tidak ada tekstur terdekode untuk diekspor.
toast-decoded = { $count } tekstur didekode
toast-decoded-not-retained = { $count } tekstur didekode, tetapi pratinjau tidak dapat dipertahankan
toast-pick-embedded-folder = Pilih folder untuk mengekspor tekstur tersemat dari { $model }
toast-basename-unknown = Tidak dapat menentukan nama dasar { $entry }
toast-read-failed = Gagal membaca { $name }: { $error }

## Toasts: import and export

toast-import-cancelled = Impor dibatalkan.
toast-imported = { $count } berkas diimpor.
toast-imported-unchecked = { $count } berkas diimpor — target validator belum ditetapkan, format tidak diperiksa.
toast-import-failed = Impor gagal: { $error }
toast-no-files-in-folder = Tidak ada berkas biasa ditemukan di { $folder }.
toast-folder-scan-failed = Pemindaian folder gagal: { $error }
toast-folder-import-failed = Impor folder gagal: { $error }
toast-folder-import-done = Impor folder selesai: { $imported } diimpor, { $skipped } dilewati, { $failed } gagal.
toast-folder-import-cancelled = Impor folder dibatalkan: { $imported } diimpor, { $skipped } dilewati, { $failed } gagal.
toast-see-log = Lihat log arsip untuk detailnya.
toast-exported = { $count } entri diekspor.
toast-export-failed = Ekspor gagal: { $error }
toast-entry-list-exported = { $count } nama entri diekspor ke { $path }.
toast-entry-list-export-failed = Ekspor daftar entri gagal: { $error }
toast-compare-failed = Perbandingan daftar entri gagal: { $error }
toast-no-missing-to-copy = Tidak ada entri hilang untuk disalin.
toast-copied-missing = { $count } nama entri yang hilang disalin.

## Toasts: clipboard, dragging and other actions

toast-copied-entry-details = Detail entri yang dipilih disalin.
toast-copied-logs = Log disalin.
toast-copied-name = Nama disalin: { $name }
toast-drag-cancelled = Penyeretan dibatalkan.
toast-moved-entries = { $count } entri dipindahkan ke arsip #{ $archive }.
toast-autoscroll = Gulir otomatis aktif: gerakkan penunjuk untuk menggulir. Klik, klik tengah, klik kanan, gunakan roda, atau tekan tombol untuk berhenti.
toast-association-added = IMG Editor Plus kini ada di “Buka dengan” File Explorer untuk .img/.dir. Untuk menjadikannya default, pilih di halaman Pengaturan yang terbuka.
toast-association-removed = Asosiasi .img/.dir dihapus.
toast-association-failed = Asosiasi berkas gagal: { $error }
toast-validation-cancelled = Validasi dibatalkan.
toast-validation-failed = Validasi gagal: { $error }
toast-no-compat-issues = Tidak ada masalah kompatibilitas — { $summary }
# $verdicts is the per-verdict count list, e.g. "native 12, untested 1".
validation-summary = Validasi untuk { $game }: { $txds } TXD ({ $textures } tekstur): { $verdicts }; { $errors } kesalahan, { $warnings } peringatan

## Archive log (the Logs box in the Export tab)

log-viewer-ready = Penampil 3D bawaan siap
log-viewer-ready-cached = Penampil 3D bawaan siap (cache)
log-viewer-opened = Penampil 3D dibuka: { $name }
log-viewer-failed = Penampil 3D gagal: { $reason }
log-viewer-closed = Penampil 3D ditutup
log-external-viewer = Membuka penampil 3D eksternal untuk { $name }
log-exported = { $count } entri diekspor
log-export-failed = Ekspor gagal: { $error }
log-entry-list-exported = Daftar entri ({ $count } nama) diekspor ke { $path }
log-compat-check = Pemeriksaan kompatibilitas: { $summary }
log-decoded = { $count } pratinjau tekstur didekode
log-texture-export-failed = Ekspor { $model } gagal: { $error }
# "Exported <what>" in the Export tab's recent list.
recent-exported = Ekspor: { $what }
recent-exported-files = { $count } berkas

## Empty workspace pro tips (they name menus and keys; keep those in step
## with the translated menu labels)

pro-tip-label = Tips:
pro-tip-search = Tekan Ctrl+F untuk memfokuskan Pencarian, lalu gunakan ↑/↓ dan Enter untuk melompat ke hasil.
pro-tip-search-context = Saran pencarian menampilkan hasil dalam konteks arsipnya; Tampilan → Konteks pilihan dalam pencarian mengaktifkan hasil terisolasi.
pro-tip-context-menu = Klik kanan entri untuk tampilan 3D, tekstur, ekspor, ganti nama, dan tindakan lain.
pro-tip-autoscroll = Klik tengah daftar entri untuk gulir otomatis gaya peramban; momentum opsional ada di Tampilan.
pro-tip-tab-keys = Tekan 1, 2, atau 3 untuk beralih ke Ekspor, Tampilan 3D, atau Tekstur.
pro-tip-close-tab = Klik tengah tab arsip untuk menutupnya dengan cepat.
pro-tip-uv-overlay = Hamparan UV tekstur tersedia saat model yang dipilih menyediakan geometri yang cocok.
pro-tip-wire-grid = Di Tampilan 3D, hamparan Wireframe menampilkan tepi segitiga dan Grid lantai membantu menilai skala.
pro-tip-save-keys = Gunakan Ctrl+S untuk menyimpan cepat dan Ctrl+Shift+S untuk menyimpan arsip dengan nama baru.
pro-tip-unique-exports = Tekstur yang diekspor memakai nama berkas unik secara otomatis, jadi ekspor massal tidak saling menimpa.
pro-tip-agr = Edit → Muat berkas animasi .agr memutar animasi di Tampilan 3D; Spasi mengalihkan pemutaran dan ←/→ melangkah per frame.
pro-tip-model-picker = Saat animasi diputar, pemilih Model di panel memutarnya ulang pada model apa pun yang kompatibel di arsip.
pro-tip-fullscreen-preview = Ikon perluas pada pratinjau impor atau penggantian membukanya layar penuh: gulir untuk memperbesar, seret untuk menggeser, Esc untuk menutup.
pro-tip-bulk-convert = Konversi pilihan ke dialek target mengodekan ulang TXD yang dipilih secara massal — pilih gamenya di Validasi tekstur terlebih dahulu.
pro-tip-entry-lists = Ctrl+L mengekspor daftar entri dan Ctrl+P membandingkannya dengan arsip untuk menemukan nama yang hilang.
toast-no-dff-to-animate = Tidak ada model DFF di arsip ini untuk dianimasikan.
toast-replace-needs-txd = Penggantian berlaku untuk entri TXD; entri itu bukan TXD.
toast-texture-replaced = Tekstur diganti — simpan arsip untuk menulisnya.
toast-bully-texture-writing = Penulisan tekstur Bully (Gamebryo) belum didukung.
toast-archive-file-missing = Berkas arsip tidak ada lagi. Gunakan Simpan sebagai… terlebih dahulu.
toast-folder-scan-target-changed = Arsip target berubah saat folder dipindai.
toast-folder-import-discarded = Impor folder dibuang karena arsip target berubah.
toast-compare-discarded = Perbandingan dibuang karena arsip berubah; coba lagi.
toast-drop-needs-archive = Buka arsip terlebih dahulu untuk menjatuhkan berkas non-IMG ke dalamnya.
toast-no-game-root = Tidak dapat menentukan folder game dari jalur arsip.
toast-textures-exported = { $count } tekstur diekspor.
toast-textures-export-failed = Ekspor tekstur gagal: { $error }
error-write-file = Gagal menulis { $path }: { $error }
error-read-entry = Gagal membaca entri: { $error }
error-texture-decode = Dekode tekstur gagal: { $error }
error-texture-preview-unsupported = Pratinjau tekstur mendukung entri TXD dan NFT; “{ $entry }” bukan wadah tekstur yang didukung.

## Entry table and search

table-name = Nama
table-size = Ukuran
# Size column for entries still read from the archive (sectors × 2 KB).
table-size-kb = { $size } KB
table-no-matches = Tidak ada entri yang cocok dengan filter saat ini.
search-label = Cari:
search-did-you-mean = Maksud Anda:
sort-tip-name-asc = Diurutkan berdasarkan nama berkas (A → Z).
sort-tip-name-desc = Diurutkan berdasarkan nama berkas (Z → A).
sort-tip-name-inactive = Urutkan berdasarkan nama berkas (A → Z).
sort-tip-type-primary = Diurutkan berdasarkan tipe berkas, { $type } lebih dulu.
sort-tip-type-alphabetical = Diurutkan berdasarkan tipe berkas secara alfabetis.
sort-tip-type-inactive = Urutkan berdasarkan tipe berkas (alfabetis).
sort-tip-size-desc = Diurutkan berdasarkan ukuran (terbesar dulu).
sort-tip-size-asc = Diurutkan berdasarkan ukuran (terkecil dulu).
sort-tip-size-inactive = Urutkan berdasarkan ukuran (terbesar dulu).
version-unknown = Tidak diketahui

## Toolbar tooltips

toolbar-new = Baru
toolbar-open = Buka
toolbar-save = Simpan
toolbar-pack = Kemas arsip
toolbar-import = Impor
toolbar-import-folder = Impor folder
toolbar-export-selected = Ekspor pilihan
toolbar-delete-selected = Hapus yang dipilih
toolbar-validate = Validasi tekstur
toolbar-image-as-txd = Impor gambar sebagai TXD
toolbar-convert-selection = Konversi pilihan ke dialek target

## Empty workspace and status bar

empty-heading = Buka atau buat arsip untuk memulai.
empty-drop-hint = Atau seret dan jatuhkan berkas .img atau .dir ke sini untuk membukanya.
status-selected = Dipilih: { $count }

## Inspector tabs

tab-export = Ekspor
tab-3d-view = Tampilan 3D
tab-texture = Tekstur

## Export tab

export-format = Format
export-entries = Entri
export-entries-value = { $total } (terlihat: { $visible })
export-game-folder = Folder game
export-game-folder-automatic = { $path } (otomatis)
export-game-folder-none = tidak ada
export-game-folder-unsaved = tidak ada (arsip belum disimpan)
export-progress = Progres
export-ready = Siap diekspor
export-open-folder = Buka folder ekspor
export-selected-entry = Entri terpilih:
export-logs = Log:
export-recent = Ekspor terbaru:
button-copy = Salin

## Entry details (Export tab, and the Copy button's clipboard text)

inspect-name = Nama
inspect-type = Tipe
inspect-size = Ukuran
inspect-offset = Offset
inspect-source = Sumber
inspect-size-mb = { $mb } MB ({ $bytes } byte, { $sectors } sektor)
inspect-size-kb = { $kb } KB ({ $bytes } byte, { $sectors } sektor)
inspect-size-bytes = { $bytes } byte ({ $sectors } sektor)
inspect-offset-value = sektor { $sector } (byte { $byte })
inspect-hex-preview = Pratinjau (hex):

## 3D view tab

viewer-no-archive = Tidak ada arsip terbuka.
viewer-try-demo = Coba demo animasi sintetis
viewer-select-model = Pilih entri .nif, .dff, atau .col untuk melihatnya dalam 3D.
viewer-gpu-unavailable = Penampil GPU tidak tersedia
viewer-gpu-hint = Coba bersihkan pratinjau atau pilih model yang lebih kecil.
viewer-clear-error = Bersihkan galat penampil
viewer-selected-model = model terpilih
viewer-preparing = Menyiapkan pratinjau 3D
viewer-preparing-detail = Membaca geometri dan mencari tekstur…
viewer-preparing-cache-note = Pratinjau model ini berikutnya akan instan.
viewer-ready = Siap melihat model ini dalam 3D.
viewer-unsupported-entry = Penampil bawaan merender entri .nif, .dff, dan .col. { $entry } bukan model yang didukung — gunakan menu klik kanan untuk penampil lain.
viewer-load-selected-hint = Gunakan “{ viewer-load-selected }” di atas untuk melihat model ini.
viewer-right-click-hint = Pilih entri .nif, .dff, atau .col, lalu klik kanan → { context-open-3d }.
viewer-toolbar-label = 3D:
viewer-preparing-selected = Menyiapkan model terpilih…
viewer-load-selected = Muat yang dipilih
viewer-load-selected-tip = Muat model yang dipilih ke penampil 3D.
viewer-reset = Atur ulang tampilan
viewer-reset-tip = Sesuaikan ulang kamera ke model. Pintasan: R
viewer-clear = Bersihkan
viewer-clear-tip = Buang adegan yang dimuat
viewer-wireframe = Wireframe
viewer-wireframe-tip = Tampilkan tepi segitiga di atas model yang diarsir.
viewer-cull = Sembunyikan sisi belakang
viewer-cull-tip = Sembunyikan segitiga sisi belakang untuk memeriksa orientasi permukaan.
viewer-textured = Bertekstur
viewer-textured-tip = Gunakan tekstur model yang terdekode alih-alih material netral.
viewer-alpha = Blending alfa
viewer-alpha-tip = Perhitungkan alfa tekstur untuk potongan dan material transparan.
viewer-alpha-unavailable-tip = Aktifkan { viewer-textured } pada model bertekstur untuk memakai blending alfa.
viewer-center = Pusatkan titik asal
viewer-center-tip = Pusatkan ulang model untuk pemeriksaan; nonaktifkan untuk mempertahankan koordinat dunia.
viewer-grid = Grid lantai
viewer-grid-tip = Tampilkan grid referensi dunia dan sumbu XYZ.
viewer-stats = { $vertices } vertex   { $triangles } segitiga   { $textures } tekstur   { $width }×{ $height }   { $orientation }   { $origin }
viewer-origin-centered = terpusat
viewer-origin-world = dunia
viewer-preparing-entry = Menyiapkan { $entry }…
viewer-no-scene = Tidak ada adegan yang dimuat

## Animation dock

anim-preparing = Menyiapkan animasi
anim-preparing-detail = Mendekode klip dan mencari tekstur…
anim-preparing-cache-note = Pemutaran ulang pasangan ini berikutnya akan instan.
anim-preparing-label = Menyiapkan { $label }…
anim-title = Animasi
anim-title-demo = Demo animasi
anim-demo-note = data sintetis — tanpa data game
anim-exit-demo = Keluar dari demo
anim-loop = Ulangi pemutaran
anim-speed = Kecepatan
anim-pack = Paket animasi
anim-model-tip = Putar ulang animasi ini pada model lain
anim-clip = Klip
anim-play = Putar (Spasi)
anim-pause = Jeda (Spasi)
anim-jump-start = Lompat ke awal (Home)
anim-step-back = Mundur satu frame (←)
anim-step-forward = Maju satu frame (→)
anim-jump-end = Lompat ke akhir (End)
anim-stop = Hentikan
anim-rate-source = sumber { $fps } fps
anim-rate-preview = pratinjau { $fps } fps
anim-frame = Bingkai
anim-frame-rest = Dasar
anim-frame-rest-tip = Bingkai pose dasar
anim-frame-pose = Pose
anim-frame-pose-tip = Bingkai pose saat ini
anim-frame-motion = Gerakan
anim-frame-motion-tip = Bingkai seluruh gerakan
anim-in-place-tip = Root di tempat — buang translasi root
anim-follow-tip = Ikuti gerakan root dengan kamera
anim-skeleton-tip = Tampilkan hamparan kerangka
anim-motion-path-tip = Tampilkan jalur gerakan root
anim-ground-tip = Tempelkan titik terendah klip ke lantai
anim-crossfade-tip = Transisi silang saat berganti klip
anim-keys-hint = Spasi putar/jeda · ←/→ langkah frame · Home/End ujung rentang · seret untuk menelusuri

## Texture tab

texture-no-archive = Tidak ada arsip terbuka.
texture-select-entry = Pilih entri TXD, NFT, NIF, atau DFF untuk melihat teksturnya.
texture-not-container = { $entry } bukan wadah tekstur. Pratinjau tersedia untuk entri TXD, NFT, atau model yang dirender.
texture-no-companions = Tidak ditemukan tekstur pendamping untuk { $entry }.
texture-load-model-hint = Muat model yang dipilih untuk menemukan teksturnya.
texture-load-model = Muat model terpilih
texture-not-decoded = { $kind } { $entry } belum didekode.
texture-load-textures = Muat tekstur { $kind } terpilih
texture-none-decodable = Tidak ada tekstur yang dapat didekode di wadah ini.
texture-animation-model = Model animasi { $entry } — ganti model di panel 3D
texture-export = Ekspor tekstur ({ $count })
texture-slot = Tekstur { $index }/{ $count }
texture-alpha = Alfa:
texture-yes = Ya
texture-no = Tidak
texture-verdict-default = Penilaian untuk target { $game }.
texture-pal8-ready = Siap PAL8 ({ $colors } warna)
texture-pal8-tip = Setiap piksel adalah salah satu dari warna berbeda ini, jadi palet 8-bit menyimpan tekstur ini tanpa kuantisasi.
texture-replace = Ganti tekstur…
texture-replace-hint = Mengimpor PNG/DDS/BMP/TGA dan mengodekan ulang untuk target arsip.
texture-uv-standalone-tip = Pratinjau tekstur mandiri. Pilih model DFF atau NIF yang cocok untuk mengaktifkan pemetaan UV.
texture-uv-unavailable-tip = Pemetaan UV tersedia setelah geometri model yang cocok dimuat.
texture-uv-tip = Tampilkan segitiga UV dari geometri model yang cocok.
texture-uv = Tampilkan peta UV
texture-uv-standalone = Pratinjau hanya tekstur · peta UV perlu geometri model yang cocok
texture-uv-unavailable = Muat geometri model yang cocok untuk mengaktifkan UV
texture-uv-triangles = { $count } segitiga
texture-only-badge = Pratinjau hanya tekstur
texture-grid = Grid
texture-grid-tip = Tampilkan grid referensi di atas pratinjau tekstur.
texture-grid-size-tip = Gunakan grid referensi { $size }×{ $size } untuk tekstur.
texture-grid-size = Ukuran:
texture-fullscreen-tip = Lihat layar penuh (kualitas penuh)
texture-model-companion = Tekstur pendamping model

## Entry table: the Type column and curated file-type names
## (the English names double as sort and grouping keys; only the
## displayed text is translated)

table-type = Tipe
# $arrow is ↑ or ↓; $primary is the file type sorted first.
table-type-sorted = Tipe { $arrow } { $primary }
file-type-model = Model
file-type-texture = Tekstur
file-type-collision = Kolisi
file-type-animation = Animasi
file-type-placement = Penempatan
file-type-definition = Definisi
file-type-data = Data
archive-untitled = Tanpa judul

## Archive log (Export tab)

log-archive-created = Arsip dibuat
log-archive-saved = Arsip disimpan
log-archive-packed = Arsip dikemas: { $entries } entri, { $reclaimed } diklaim kembali
log-imported-entries = { $count } entri diimpor
log-folder-import = Impor folder: { $imported } diimpor, { $skipped } dilewati, { $failed } gagal
log-folder-import-detail = Detail impor folder: { $detail }
folder-import-cancelled-detail = Impor dibatalkan; berkas yang tersisa dilewati.
folder-import-duplicate = { $name }: duplikat dilewati
entry-name-empty = nama kosong
entry-name-not-ascii = nama berisi karakter yang tidak dapat dibaca game (gunakan A–Z, 0–9, dan tanda baca dasar)
entry-name-too-long = nama melebihi { $limit } byte
import-skip-no-extension = berkas tidak memiliki ekstensi
import-skip-not-file = bukan berkas biasa
log-import-skipped = { $name } dilewati: { $reason }
toast-rename-rejected = Tidak dapat mengganti nama: { $reason }

## Entry details: source and format summary

inspect-source-imported = Diimpor
inspect-source-imported-from = Diimpor dari { $path }
inspect-source-archive = Arsip { $archive } pada sektor { $sector }
inspect-key-format = Format
inspect-key-version = Versi
inspect-key-clump-size = Ukuran clump
inspect-key-endian = Endian
inspect-key-user-version = Versi pengguna
inspect-key-lines = Baris
inspect-key-atomics = Atomics
inspect-key-textures = Tekstur
inspect-key-entries = Entri
inspect-key-mesh-vertices = Vertex mesh
inspect-key-mesh-faces = Sisi mesh
inspect-key-spheres = Bola
inspect-key-boxes = Kotak
inspect-key-shadow-mesh = Mesh bayangan
inspect-rw-truncated = RenderWare (terpotong)
inspect-rw-clump = Clump (model)
inspect-rw-txd = Kamus Tekstur
inspect-rw-pi-txd = Kamus Tekstur independen platform
inspect-rw-animation = Animasi
inspect-rw-uv-animation = Animasi UV
inspect-rw-stream = Aliran RenderWare
inspect-bytes = { $count } byte
inspect-col-unknown = Kolisi tidak diketahui
inspect-endian-big = Big-endian
inspect-endian-little = Little-endian
inspect-nif-truncated = NIF (terpotong)
inspect-lines-value = { $lines } ({ $nonempty } tidak kosong)
inspect-format-scm = Skrip GTA (main.scm)
inspect-format-ipl = Penempatan item GTA
inspect-format-ide = Definisi item GTA
inspect-texture-count = { $count } tekstur

## Embedded-texture export (NIF → NFT)

texture-export-none = Tidak ada tekstur tersemat yang ditemukan
texture-export-done = { $count } tekstur tersemat diekspor
texture-export-partial = Tekstur diekspor: { $written }, gagal: { $failures }
texture-export-no-nft = Tidak ada NFT ditemukan untuk “{ $name }”

## Manifest comparison errors

compare-manifest-inspect = Tidak dapat memeriksa manifes “{ $path }”: { $error }
compare-manifest-too-large = Manifes “{ $path }” terlalu besar ({ $size }; batasnya { $limit }).
compare-manifest-read = Tidak dapat membaca manifes “{ $path }”: { $error }
compare-manifest-grew = Manifes “{ $path }” melampaui batas { $limit } saat dibaca.
compare-manifest-utf8 = Manifes “{ $path }” bukan UTF-8 yang valid: { $error }

## Compatibility verdicts (lowercase: they appear mid-sentence)

verdict-native = asli
verdict-supported = didukung
verdict-convertible-lossless = dapat dikonversi (tanpa kehilangan)
verdict-convertible-lossy = dapat dikonversi (dengan kehilangan)
verdict-unsupported = tidak didukung
verdict-untested = belum diuji
verdict-count = { $verdict }: { $count }

## Compatibility evidence notes
## These cite measurements of the retail games. Keep format names
## (DXT1, PAL8, X8R8G8B8, D3D8, pp=1 …) and file names untranslated.
## "Raster" is one texture image inside a TXD; "dialect" is the set of
## texture formats a game's own files use.

compat-class-other-nif = format NiPixelData lainnya
compat-cat-iii-pal = 96,5% tekstur dunia retail
compat-cat-iii-888 = 6.806 raster, termasuk set pemain/kendaraan
compat-cat-iii-8888 = 1.121 raster
compat-cat-iii-1555 = 24 raster
compat-cat-iii-dxt1 = retail tidak menyertakan satu pun; perangkat keras D3D8 mendukungnya
compat-cat-iii-dxt = nilai kompresi D3D8 1-5; retail tidak menyertakan satu pun
compat-cat-depth24 = bentuk kedalaman 24 yang didokumentasikan; stride/urutan belum diverifikasi
compat-cat-iii-565 = dipetakan driver; III tidak menyertakan satu pun
compat-cat-555-lum8 = driver memetakan C555 dan LUM8; retail tidak menyertakan satu pun
compat-cat-a8l8 = D3D9/dekoder mendukungnya; jalur nibble RW belum diverifikasi
compat-cat-vc-dxt1 = dialek dunia retail; D3D8 pp=1 (10 ribu+ raster, nibble usang)
compat-cat-vc-dxt3 = dialek alfa retail; D3D8 pp=3 (1.149 raster)
compat-cat-vc-pal = 27 raster; diterima tetapi jarang
compat-cat-vc-888 = 1 raster
compat-cat-vc-8888 = III menyertakannya; VC sendiri tidak menyertakan satu pun
compat-cat-vc-565 = label 565 retail adalah data DXT1; tidak ada R565 asli yang terukur
compat-cat-vc-16bit = label retail adalah data DXT1/DXT3; bentuk 16-bit mentah belum terukur
compat-cat-vc-dxt = nilai kompresi D3D8 ada; retail hanya menyertakan 1 dan 3
compat-cat-sa-dxt1 = 28.807 raster di empat arsip
compat-cat-sa-dxt3 = 2.098 raster
compat-cat-sa-888 = 1.015 raster, sebagian besar skin player.img
compat-cat-sa-8888 = 237 raster
compat-cat-sa-dxt5 = retail tidak menyertakan satu pun; D3D9 mendukungnya
compat-cat-sa-dxt24 = kata format D3D9 membawanya; alfa premultiplied
compat-cat-sa-pal = sumber bertentangan; retail tidak menyertakan satu pun; urai dan pertahankan
compat-cat-sa-16bit = dipetakan driver; retail tidak menyertakan 16-bit tak terkompresi
compat-cat-sa-a8l8 = Magic.TXD mencantumkannya untuk SA PC; jalur RW belum diverifikasi
compat-cat-bully-dxt1 = 31.714 raster
compat-cat-bully-dxt5 = 3.526 raster
compat-cat-bully-rgb = 138 / 134 raster
compat-cat-bully-pal = 127 / 1 raster
compat-cat-bully-dxt3 = Gamebryo mendukungnya; retail tidak menyertakan satu pun
compat-cat-bully-other = 15 raster tidak dapat didekode

compat-note-bully-not-rw = aset Bully adalah Gamebryo NIF/NFT, bukan native RenderWare
compat-note-platform-rewrite = raster platform-{ $platform } dalam arsip { $game } memerlukan penulisan ulang platform/versi
compat-note-no-profile = belum ada tabel profil
compat-note-nft-not-rw = raster NFT Gamebryo bukan native RenderWare
compat-note-bully-dxt1 = Bully retail: 31.714 raster DXT1
compat-note-bully-dxt5 = Bully retail: 3.526 raster DXT5
compat-note-bully-rgb = Bully retail menyertakan RGB/RGBA mentah (138/134)
compat-note-bully-pal = Bully retail menyertakan raster berpalet (127 PAL + 1 PALA)
compat-note-bully-dxt3 = Gamebryo mendukung DXT3 tetapi Bully retail tidak menyertakan satu pun
compat-note-sa-dxt = SA retail: 28.807 DXT1 + 2.098 DXT3 raster
compat-note-sa-dxt24 = format native D3D9 membawa DXT2/DXT4 (premultiplied); SA retail tidak menyertakan satu pun
compat-note-sa-dxt5 = SA retail tidak menyertakan satu pun; DXT5 mengandalkan dukungan D3D9 (perkakas mod memakainya)
compat-note-sa-pal = sumber bertentangan soal palet SA; SA retail tidak menyertakan satu pun; urai dan pertahankan
compat-note-sa-depth24 = bentuk R8G8B8 kedalaman 24 yang didokumentasikan; SA retail tidak menyertakan satu pun; runtime belum diverifikasi
compat-note-sa-16bit = driver memetakan 1555/565/4444; SA retail tidak menyertakan 16-bit tak terkompresi
compat-note-c555 = driver memetakan C555 ke X1R5G5B5; retail tidak menyertakan satu pun
compat-note-lum8 = driver memetakan LUM8 ke D3DFMT_L8; retail tidak menyertakan satu pun
compat-note-sa-a8l8 = D3D9 membawa A8L8 dan dekoder independen mendukungnya; pemetaan nibble RW biasa belum diverifikasi
compat-note-iii-pal = III retail: 96,5% PAL8; VC retail: 27 raster — diterima tetapi jarang
compat-note-vc-dxt1 = dialek dunia VC retail: DXT1 dengan D3D8 pp=1; nibble raster usang
compat-note-iii-dxt1 = III retail tidak menyertakan raster terkompresi (0/15.372); perangkat keras D3D8 mendukung DXT1
compat-note-vc-dxt3 = dialek alfa VC retail: DXT3 dengan D3D8 pp=3 (1.149 raster)
compat-note-iii-dxt = nilai kompresi D3D8 1-5 memetakan ke DXT1-5 (DXT2/4 premultiplied); III+VC retail hanya menyertakan 1 dan 3
compat-note-iii-depth24 = bentuk R8G8B8 kedalaman 24 yang didokumentasikan; stride/urutan/runtime belum diverifikasi
compat-note-iii-8888 = III retail menyertakan 8888 di kedua arsip (1.121 raster)
compat-note-vc-565 = raster berlabel 565 VC retail adalah data DXT1; tidak ada R565 asli yang terukur
compat-note-iii-565 = bentuk 16-bit dipetakan driver; III tidak menyertakan satu pun
compat-note-vc-1555 = label 1555 VC retail adalah data DXT1; tidak ada R1555 asli yang terukur
compat-note-vc-4444 = label 4444 VC retail adalah data DXT3; tidak ada R4444 asli yang terukur
compat-note-iii-4444 = III tidak menyertakan satu pun; 16-bit dengan alfa era D3D8
compat-note-iii-a8l8 = Magic.TXD mencantumkan A8L8 untuk SA PC; D3D9 membawanya; jalur RW biasa belum diverifikasi

## Output-format choices (import and replace dialogs)

compat-choice-iii-888 = X8R8G8B8 32-bit, tanpa kehilangan; standar txd.img III retail
compat-choice-iii-8888 = A8R8G8B8, mempertahankan alfa; III retail menyertakan 1.121
compat-choice-iii-pal8 = palet 8-bit, mengkuantisasi warna; dialek dunia retail (96,5%)
compat-choice-pal4 = palet 4-bit, kuantisasi ketat; hanya karya 16 warna
compat-choice-iii-1555 = 16-bit dengan alfa 1-bit; retail menyertakan 24
compat-choice-iii-dxt = didukung perangkat keras tetapi tidak dipakai III; dengan kehilangan
compat-choice-vc-dxt1 = dialek dunia VC retail (D3D8 pp=1); kompresi dengan kehilangan
compat-choice-vc-dxt3 = dialek alfa VC retail (D3D8 pp=3); kompresi dengan kehilangan
compat-choice-vc-888 = X8R8G8B8 32-bit, tanpa kehilangan; VC retail menyertakan satu
compat-choice-vc-8888 = A8R8G8B8, mempertahankan alfa; bentuk era D3D8
compat-choice-vc-pal8 = palet 8-bit, mengkuantisasi warna; VC retail menyertakan 27
compat-choice-vc-565 = 16-bit; VC retail berlabel 565 sebagai data DXT, bentuk mentah belum terukur
compat-choice-vc-4444 = 16-bit dengan alfa; VC retail berlabel 4444 sebagai data DXT3
compat-choice-player-888 = X8R8G8B8 32-bit; player.img retail menyertakan 269
compat-choice-player-8888 = A8R8G8B8, mempertahankan alfa; player.img retail menyertakan 125
compat-choice-player-dxt = didukung di mana-mana; dengan kehilangan (player.img tidak menyertakan satu pun)
compat-choice-sa-dxt1 = dialek dunia SA retail; kompresi dengan kehilangan
compat-choice-sa-dxt3 = dialek alfa SA retail; kompresi dengan kehilangan
compat-choice-sa-8888 = A8R8G8B8, tanpa kehilangan; SA retail menyertakannya di player.img
compat-choice-sa-pal8 = mengkuantisasi warna; SA retail tidak menyertakan raster berpalet

## Conversion warnings and errors

compat-warn-alpha-discarded = { $source } memiliki alfa, tetapi { $format } tidak dapat menyimpannya — kanal alfa akan dibuang.
compat-warn-dxt-lossy = Kompresi { $format } menyebabkan kehilangan; pratinjau menampilkan hasil terkode.
compat-warn-palette-exact = { $format } menyimpan gambar dengan tepat ({ $colors } warna).
compat-warn-palette-quantize = Warna akan dikuantisasi menjadi paling banyak { $cap } entri.
compat-warn-stream-budget = { $width }x{ $height } melebihi anggaran streaming 1024 px SA; game mungkin tidak dapat memuatnya secara streaming.
# $verdict is one of the verdict-* labels above.
compat-warn-verdict = { $format } { $verdict } untuk { $game }: { $note }
compat-error-image-format = format gambar tidak dikenal: { $error }
compat-error-image-kind = gambar { $kind } tidak didukung; gunakan PNG, DDS, BMP, atau TGA
compat-error-decode = dekode { $label } gagal: { $error }
compat-error-image-size = ukuran gambar tidak didukung { $width }x{ $height }
compat-error-unreadable-texture = “{ $name }” tidak dapat didekode ({ $error }); konversi memerlukan piksel yang terbaca
compat-error-no-dimensions = tekstur tidak memiliki dimensi
compat-error-texture-index = indeks tekstur { $index } di luar rentang
compat-error-unknown-target = target game tidak dikenal “{ $id }”

## Save check: container conventions

compat-container-v2-expects-v1 = wadah IMG v2, tetapi { $game } mengharapkan IMG v1 — game tidak akan melihat berkas ini.
compat-container-v1-sa = wadah IMG v1; San Andreas retail memakai IMG v2 (v1 hanya dimuat jika tercantum di gta.dat).
compat-container-xbox = pengemasan Xbox 360; { $game } mengharapkan wadah PC.

## Import check notes

compat-scan-unreadable = tidak dapat membaca: { $error }
compat-scan-bad-nif = header NIF tidak terbaca: { $error }
compat-scan-empty-nif = tidak ada blok NiPixelData (stub kosong)
compat-scan-not-texture = bukan tekstur TXD atau Gamebryo

## Target-game suggestion evidence ($share is a percentage such as "92%")

compat-hint-gamebryo = { $total } entri Gamebryo ({ $nft } NFT, { $nif } NIF)
compat-hint-d3d9 = platform 9 (D3D9) pada { $share } dari { $total } raster sampel
compat-hint-pal = PAL8/PAL4 pada { $share } raster sampel
compat-hint-vc16 = 16-bit 565/4444/1555 pada { $share } raster sampel
compat-hint-d3d8 = platform 8 (D3D8)
