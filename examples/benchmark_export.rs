use std::path::{Path, PathBuf};
use std::time::Instant;

use imgeditor::archive::ArchiveInfo;
use imgeditor::tasks::{ExportMode, ExportTask};

fn run_export(img_path: &Path, out_dir: &Path) -> (u64, u64, u64) {
    let _ = std::fs::remove_dir_all(out_dir);
    std::fs::create_dir_all(out_dir).expect("failed to create output dir");

    let open_start = Instant::now();
    let archive = ArchiveInfo::open(img_path).expect("failed to open archive");
    let open_elapsed = open_start.elapsed().as_millis() as u64;

    let export_start = Instant::now();
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let task = ExportTask::new(archive, out_dir.to_path_buf(), ExportMode::All);
    rt.block_on(task.run()).expect("export failed");
    let export_elapsed = export_start.elapsed().as_millis() as u64;

    let total_elapsed = open_elapsed + export_elapsed;
    (open_elapsed, export_elapsed, total_elapsed)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let mut img_path = PathBuf::from("C:/Games/Bully - Scholarship Edition/Stream/World.img");
    let mut out_dir = PathBuf::from("C:/Temp/imgeditor_rust_export");
    let mut iterations = 1;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-i" | "--input" => {
                i += 1;
                if i < args.len() {
                    img_path = PathBuf::from(&args[i]);
                }
            }
            "-o" | "--output" => {
                i += 1;
                if i < args.len() {
                    out_dir = PathBuf::from(&args[i]);
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

    println!("Rust IMGEditor export benchmark");
    println!("input:  {}", img_path.display());
    println!("output: {}", out_dir.display());
    println!("iterations: {}", iterations);

    let mut open_times = Vec::new();
    let mut export_times = Vec::new();
    let mut total_times = Vec::new();

    for iter in 1..=iterations {
        println!("\n--- iteration {iter} ---");
        let (open_ms, export_ms, total_ms) = run_export(&img_path, &out_dir);
        println!("open:   {:.3} s", open_ms as f64 / 1000.0);
        println!("export: {:.3} s", export_ms as f64 / 1000.0);
        println!("total:  {:.3} s", total_ms as f64 / 1000.0);
        open_times.push(open_ms);
        export_times.push(export_ms);
        total_times.push(total_ms);
    }

    open_times.sort_unstable();
    export_times.sort_unstable();
    total_times.sort_unstable();

    let median = |v: &mut [u64]| {
        let n = v.len();
        if n % 2 == 1 {
            v[n / 2]
        } else {
            (v[n / 2 - 1] + v[n / 2]) / 2
        }
    };

    println!("\n=== median ===");
    println!("open:   {:.3} s", median(&mut open_times) as f64 / 1000.0);
    println!("export: {:.3} s", median(&mut export_times) as f64 / 1000.0);
    println!("total:  {:.3} s", median(&mut total_times) as f64 / 1000.0);
}
