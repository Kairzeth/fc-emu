use fc_emu::{
    DEFAULT_ROM_PATH,
    emulator::Emulator,
    input::Button,
    rom::Rom,
    window::{NES_HEIGHT, NES_WIDTH},
};
use std::{env, fs, io::Write, path::Path};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let rom_path = args.next().unwrap_or_else(|| DEFAULT_ROM_PATH.to_string());
    let output_path = args
        .next()
        .unwrap_or_else(|| "/private/tmp/fc-emu-frame.ppm".to_string());
    let frames: usize = args
        .next()
        .and_then(|value| value.parse().ok())
        .unwrap_or(120);
    let options: Vec<String> = args.collect();
    let press_start = options.iter().any(|arg| arg == "--start");
    let play_script = options.iter().any(|arg| arg == "--play");

    let rom = Rom::from_path(&rom_path)?;
    let rom_name = Path::new(&rom_path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("rom")
        .to_string();
    let mut emulator = Emulator::new(rom, rom_name)?;

    for frame in 0..frames {
        if press_start && frame == 90 {
            emulator.set_button(Button::Start, true);
        }
        if press_start && frame == 98 {
            emulator.set_button(Button::Start, false);
        }
        if play_script {
            apply_play_script(&mut emulator, frame);
        }
        emulator.step_frame();
    }

    write_ppm(&output_path, emulator.frame_buffer())?;
    let cpu = emulator.cpu_state();
    println!(
        "cpu pc=${:04x} stopped={} cycles={}",
        cpu.pc, cpu.stopped, cpu.cycles
    );
    println!("wrote {output_path}");
    Ok(())
}

fn apply_play_script(emulator: &mut Emulator, frame: usize) {
    if frame == 90 {
        emulator.set_button(Button::Start, true);
    }
    if frame == 98 {
        emulator.set_button(Button::Start, false);
    }
    if frame == 170 {
        emulator.set_button(Button::Right, true);
        emulator.set_button(Button::B, true);
    }
    if frame == 235 {
        emulator.set_button(Button::A, true);
    }
    if frame == 255 {
        emulator.set_button(Button::A, false);
    }
    if frame == 420 {
        emulator.set_button(Button::Right, false);
        emulator.set_button(Button::B, false);
    }
}

fn write_ppm(path: &str, rgba: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    let mut file = fs::File::create(path)?;
    writeln!(file, "P6\n{} {}\n255", NES_WIDTH, NES_HEIGHT)?;
    for pixel in rgba.chunks_exact(4) {
        file.write_all(&pixel[..3])?;
    }
    Ok(())
}
