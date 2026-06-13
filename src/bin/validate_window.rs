use fc_emu::{
    app::App,
    select_rom_path,
    window::{WindowValidationConfig, run_validation},
};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

fn main() -> anyhow::Result<ExitCode> {
    tracing_subscriber::fmt::init();

    let args: Vec<String> = env::args().collect();
    let rom_path = select_rom_path(args.clone());
    let frames = args
        .get(2)
        .and_then(|value| value.parse().ok())
        .unwrap_or_else(|| WindowValidationConfig::default().max_frames);

    let app = App::new(&rom_path)?;
    let report = run_validation(
        app,
        WindowValidationConfig {
            max_frames: frames,
            trace_path: Some(trace_path()),
            ..WindowValidationConfig::default()
        },
    )?;

    println!(
        "window validation: passed={} frames={} stagnant_frames={} cpu_pc=${:04x} stopped={} reason={}",
        report.passed,
        report.frames,
        report.stagnant_frames,
        report.cpu_pc,
        report.cpu_stopped,
        report.reason
    );
    fs::write(
        report_path(),
        format!(
            "passed={}\nframes={}\nstagnant_frames={}\ncpu_pc=${:04x}\ncpu_cycles={}\nstopped={}\nppu_status=${:02x}\nscroll_x={}\nscroll_y={}\nram_digest={:016x}\noam_digest={:016x}\nreason={}\n",
            report.passed,
            report.frames,
            report.stagnant_frames,
            report.cpu_pc,
            report.cpu_cycles,
            report.cpu_stopped,
            report.ppu_status,
            report.scroll_x,
            report.scroll_y,
            report.ram_digest,
            report.oam_digest,
            report.reason
        ),
    )?;

    Ok(if report.passed {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

fn trace_path() -> PathBuf {
    dist_path().join("window-validation-trace.csv")
}

fn report_path() -> PathBuf {
    dist_path().join("window-validation-report.txt")
}

fn dist_path() -> PathBuf {
    if let Ok(exe) = env::current_exe() {
        for ancestor in exe.ancestors() {
            if ancestor.extension().and_then(|ext| ext.to_str()) == Some("app")
                && let Some(dist) = ancestor.parent()
            {
                return dist.to_path_buf();
            }
        }
    }

    Path::new("dist").to_path_buf()
}
