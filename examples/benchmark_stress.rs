use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Instant;

use crossbeam::channel::{bounded, Receiver, Sender};
use imgeditor::archive::ArchiveInfo;
use imgeditor::archive::EntryInfo;
use imgeditor::parser::{ImgVersion, SECTOR_SIZE};
use rayon::prelude::*;

#[derive(Debug, Clone, Copy)]
enum Mode {
    ParallelMemmap,
    SequentialMemmap,
    BufferedSingleFile,
    BatchedParallel { workers: usize },
    BatchedParallelSubdirs { workers: usize },
    SortedBufferedPool { workers: usize, chunk_mb: usize },
}

fn export_parallel_memmap(archive: &ArchiveInfo, out_dir: &Path) -> u64 {
    let entries: Vec<_> = archive.entries.iter().collect();
    let export_start = Instant::now();
    entries.par_iter().for_each(|entry| {
        let output_path = out_dir.join(&entry.file_name);
        imgeditor::parser::export_entry_to_file(archive, entry, &output_path)
            .expect("export failed");
    });
    export_start.elapsed().as_millis() as u64
}

fn export_sequential_memmap(archive: &ArchiveInfo, out_dir: &Path) -> u64 {
    let export_start = Instant::now();
    for entry in &archive.entries {
        let output_path = out_dir.join(&entry.file_name);
        imgeditor::parser::export_entry_to_file(archive, entry, &output_path)
            .expect("export failed");
    }
    export_start.elapsed().as_millis() as u64
}

fn export_buffered_single_file(archive: &ArchiveInfo, out_dir: &Path) -> u64 {
    let source_path = archive.path.as_ref().expect("no source path");
    let file = File::open(source_path).expect("open archive");
    let mut reader = BufReader::with_capacity(1024 * 1024, file);

    let export_start = Instant::now();
    for entry in &archive.entries {
        let output_path = out_dir.join(&entry.file_name);
        let output_path = imgeditor::parser::unique_output_path(&output_path);
        let size = u64::from(entry.sector) * SECTOR_SIZE;
        let offset = u64::from(entry.offset) * SECTOR_SIZE;

        let mut data = vec![0u8; size as usize];
        reader
            .seek(SeekFrom::Start(offset))
            .expect("seek");
        reader.read_exact(&mut data).expect("read");
        std::fs::write(&output_path, data).expect("write");
    }
    export_start.elapsed().as_millis() as u64
}

fn export_batched_parallel(
    archive: &ArchiveInfo,
    out_dir: &Path,
    workers: usize,
    into_subdirs: bool,
) -> u64 {
    let source_path = archive.path.clone().expect("no source path");
    let entries: Vec<_> = archive.entries.clone();

    let chunks: Vec<Vec<_>> = entries
        .chunks((entries.len() / workers).max(1) + 1)
        .map(|c| c.to_vec())
        .collect();

    let export_start = Instant::now();
    chunks.into_par_iter().enumerate().for_each(|(idx, chunk)| {
        let chunk_dir = if into_subdirs {
            let d = out_dir.join(format!("batch_{}", idx));
            let _ = std::fs::create_dir_all(&d);
            d
        } else {
            out_dir.to_path_buf()
        };

        let file = File::open(&source_path).expect("open archive");
        let mut reader = BufReader::with_capacity(4 * 1024 * 1024, file);
        for entry in chunk {
            let output_path = chunk_dir.join(&entry.file_name);
            let output_path = imgeditor::parser::unique_output_path(&output_path);
            let size = u64::from(entry.sector) * SECTOR_SIZE;
            let offset = u64::from(entry.offset) * SECTOR_SIZE;

            let mut data = vec![0u8; size as usize];
            reader.seek(SeekFrom::Start(offset)).expect("seek");
            reader.read_exact(&mut data).expect("read");
            std::fs::write(&output_path, data).expect("write");
        }
    });
    export_start.elapsed().as_millis() as u64
}

fn export_sorted_buffered_pool(
    archive: &ArchiveInfo,
    out_dir: &Path,
    writers: usize,
    chunk_mb: usize,
) -> u64 {
    let source_path = archive.path.clone().expect("no source path");
    let entries: Vec<EntryInfo> = archive.entries.clone();

    let chunk_size = chunk_mb * 1024 * 1024;
    let (buf_tx, buf_rx): (Sender<(PathBuf, Vec<u8>, usize)>, Receiver<(PathBuf, Vec<u8>, usize)>) =
        bounded(writers * 2);
    let (free_tx, free_rx): (Sender<Vec<u8>>, Receiver<Vec<u8>>) = bounded(writers * 2);

    for _ in 0..writers.max(1) {
        let _ = free_tx.send(vec![0u8; chunk_size]);
    }

    let writer_handles: Vec<_> = (0..writers.max(1))
        .map(|_| {
            let rx = buf_rx.clone();
            let free = free_tx.clone();
            thread::spawn(move || {
                while let Ok((path, mut buf, len)) = rx.recv() {
                    let path = imgeditor::parser::unique_output_path(&path);
                    if let Some(parent) = path.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    if let Err(e) = std::fs::write(&path, &buf[..len]) {
                        eprintln!("write failed for {}: {}", path.display(), e);
                    }
                    buf.clear();
                    let _ = free.send(buf);
                }
            })
        })
        .collect();

    let export_start = Instant::now();

    let mut sorted: Vec<&EntryInfo> = entries.iter().filter(|e| !e.imported).collect();
    sorted.sort_by_key(|e| e.offset);

    let mut file = File::open(&source_path).expect("open archive");
    let mut current_offset: u64 = 0;

    for entry in sorted {
        let size = (entry.sector as usize) * SECTOR_SIZE as usize;
        let offset = (entry.offset as u64) * SECTOR_SIZE;

        let mut buf = free_rx.recv().expect("free buffer");
        if buf.capacity() < size {
            buf = vec![0u8; size];
        }
        buf.resize(size, 0);

        if offset != current_offset {
            file.seek(SeekFrom::Start(offset)).expect("seek");
            current_offset = offset;
        }
        file.read_exact(&mut buf).expect("read");
        current_offset += size as u64;

        let first = entry
            .file_name
            .chars()
            .next()
            .unwrap_or('_')
            .to_string();
        let bucket_dir = out_dir.join(first);
        let path = bucket_dir.join(&entry.file_name);

        buf_tx.send((path, buf, size)).expect("send to writer");
    }

    drop(buf_tx);
    drop(free_tx);
    for h in writer_handles {
        let _ = h.join();
    }

    export_start.elapsed().as_millis() as u64
}

fn run_benchmark(img_path: &Path, out_dir: &Path, mode: Mode, iterations: usize) -> (u64, u64, u64) {
    let mut open_times = Vec::new();
    let mut export_times = Vec::new();
    let mut total_times = Vec::new();

    for _ in 0..iterations {
        let _ = std::fs::remove_dir_all(out_dir);
        std::fs::create_dir_all(out_dir).expect("create out dir");

        let open_start = Instant::now();
        let archive = ArchiveInfo::open(img_path).expect("open archive");
        let open_ms = open_start.elapsed().as_millis() as u64;

        let export_ms = match mode {
            Mode::ParallelMemmap => export_parallel_memmap(&archive, out_dir),
            Mode::SequentialMemmap => export_sequential_memmap(&archive, out_dir),
            Mode::BufferedSingleFile => export_buffered_single_file(&archive, out_dir),
            Mode::BatchedParallel { workers } => {
                export_batched_parallel(&archive, out_dir, workers, false)
            }
            Mode::BatchedParallelSubdirs { workers } => {
                export_batched_parallel(&archive, out_dir, workers, true)
            }
            Mode::SortedBufferedPool { workers, chunk_mb } => {
                export_sorted_buffered_pool(&archive, out_dir, workers, chunk_mb)
            }
        };

        open_times.push(open_ms);
        export_times.push(export_ms);
        total_times.push(open_ms + export_ms);
    }

    let median = |v: &mut [u64]| {
        v.sort_unstable();
        let n = v.len();
        if n % 2 == 1 {
            v[n / 2]
        } else {
            (v[n / 2 - 1] + v[n / 2]) / 2
        }
    };

    (
        median(&mut open_times),
        median(&mut export_times),
        median(&mut total_times),
    )
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut img_path = PathBuf::from("C:/Games/Bully - Scholarship Edition/Stream/World.img");
    let mut iterations = 3;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-i" | "--input" => {
                i += 1;
                if i < args.len() {
                    img_path = PathBuf::from(&args[i]);
                }
            }
            "-n" | "--iterations" => {
                i += 1;
                if i < args.len() {
                    iterations = args[i].parse().expect("iterations must be a number");
                }
            }
            _ => {}
        }
        i += 1;
    }

    if !img_path.exists() {
        eprintln!("Input archive not found: {}", img_path.display());
        std::process::exit(1);
    }

    let archive = ArchiveInfo::open(&img_path).expect("open archive");
    let entry_count = archive.entries.len();
    let total_mb: u64 = archive.entries.iter().map(|e| u64::from(e.sector) * SECTOR_SIZE).sum();
    println!("Rust IMGEditor export stress test");
    println!("input:  {}", img_path.display());
    println!("entries: {}", entry_count);
    println!("total bytes: {} MB", total_mb / 1024 / 1024);
    println!("iterations per mode: {}", iterations);
    println!();

    let modes = vec![
        ("parallel+memmap", Mode::ParallelMemmap),
        ("sequential+memmap", Mode::SequentialMemmap),
        ("buffered+single-file", Mode::BufferedSingleFile),
        ("batched-parallel-4", Mode::BatchedParallel { workers: 4 }),
        ("sorted-buffered-pool-4", Mode::SortedBufferedPool { workers: 4, chunk_mb: 32 }),
    ];

    println!("{:<24} {:>10} {:>10} {:>10}", "mode", "open(ms)", "export(s)", "total(s)");
    for (name, mode) in modes {
        let out_dir = PathBuf::from(format!("C:/Temp/imgeditor_rust_stress_{}", name.replace('+', "_")));
        let (open_ms, export_ms, total_ms) = run_benchmark(&img_path, &out_dir, mode, iterations);
        println!(
            "{:<24} {:>10} {:>10.3} {:>10.3}",
            name,
            open_ms,
            export_ms as f64 / 1000.0,
            total_ms as f64 / 1000.0
        );
    }
}
