use crate::{
    app::App,
    input::{AppControlAction, Button, KeyboardKey, SaveSlot},
};
use anyhow::{Context, Result};
use cpal::{
    SampleFormat, Stream, StreamConfig,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use pixels::{Pixels, SurfaceTexture};
use std::{
    collections::VecDeque,
    fs::File,
    io::Write,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tracing::{error, info, warn};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{ElementState, KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, KeyCode, NamedKey, PhysicalKey},
    window::{Window, WindowId},
};

pub const NES_WIDTH: u32 = 256;
pub const NES_HEIGHT: u32 = 240;
pub const DEFAULT_SCALE: u32 = 3;
const TARGET_FRAME_TIME: Duration = Duration::from_micros(16_667);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WindowConfig {
    pub width: u32,
    pub height: u32,
    pub scale: u32,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            width: NES_WIDTH,
            height: NES_HEIGHT,
            scale: DEFAULT_SCALE,
        }
    }
}

impl WindowConfig {
    pub fn content_size(&self) -> (u32, u32) {
        (self.width * self.scale, self.height * self.scale)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WindowValidationConfig {
    pub max_frames: u64,
    pub stale_frame_limit: u64,
    pub trace_path: Option<std::path::PathBuf>,
}

impl Default for WindowValidationConfig {
    fn default() -> Self {
        Self {
            max_frames: 1_200,
            stale_frame_limit: 180,
            trace_path: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WindowValidationReport {
    pub frames: u64,
    pub stagnant_frames: u64,
    pub cpu_pc: u16,
    pub cpu_cycles: u64,
    pub cpu_stopped: bool,
    pub ppu_status: u8,
    pub scroll_x: u16,
    pub scroll_y: u16,
    pub ram_digest: u64,
    pub oam_digest: u64,
    pub passed: bool,
    pub reason: String,
}

pub fn run(app: App) -> Result<()> {
    run_inner(app, None).map(|_| ())
}

pub fn run_validation(app: App, config: WindowValidationConfig) -> Result<WindowValidationReport> {
    let validation = run_inner(app, Some(ValidationState::new(config)))?;
    validation.context("validation mode exited without a report")
}

fn run_inner(
    app: App,
    validation: Option<ValidationState>,
) -> Result<Option<WindowValidationReport>> {
    let event_loop = EventLoop::new().context("failed to create winit event loop")?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let audio = AudioOutput::new();
    let mut runtime = RuntimeApp {
        app,
        config: WindowConfig::default(),
        window: None,
        pixels: None,
        audio,
        next_frame_at: Instant::now(),
        runtime_trace: RuntimeTrace::new_for_current_app().ok(),
        validation,
    };

    event_loop
        .run_app(&mut runtime)
        .context("winit event loop failed")?;
    Ok(runtime.validation.and_then(|validation| validation.report))
}

struct RuntimeApp {
    app: App,
    config: WindowConfig,
    window: Option<Arc<Window>>,
    pixels: Option<Pixels<'static>>,
    audio: AudioOutput,
    next_frame_at: Instant,
    runtime_trace: Option<RuntimeTrace>,
    validation: Option<ValidationState>,
}

impl RuntimeApp {
    fn create_window(&mut self, event_loop: &ActiveEventLoop) -> Result<()> {
        if self.window.is_some() {
            return Ok(());
        }

        let (width, height) = self.config.content_size();
        let attrs = Window::default_attributes()
            .with_title(self.app.window_title())
            .with_inner_size(LogicalSize::new(f64::from(width), f64::from(height)))
            .with_min_inner_size(LogicalSize::new(
                f64::from(NES_WIDTH),
                f64::from(NES_HEIGHT),
            ));
        let window = Arc::new(
            event_loop
                .create_window(attrs)
                .context("failed to create application window")?,
        );
        window.set_ime_allowed(false);
        window.focus_window();
        let size = window.inner_size();
        let surface = SurfaceTexture::new(size.width.max(1), size.height.max(1), window.clone());
        let pixels = Pixels::new(NES_WIDTH, NES_HEIGHT, surface)
            .context("failed to create pixels framebuffer")?;

        info!(
            width = size.width,
            height = size.height,
            "created emulator window"
        );
        self.pixels = Some(pixels);
        self.window = Some(window);
        self.next_frame_at = Instant::now();
        Ok(())
    }

    fn redraw(&mut self, event_loop: &ActiveEventLoop) {
        let Some(window) = self.window.as_ref() else {
            return;
        };
        let Some(pixels) = self.pixels.as_mut() else {
            return;
        };

        if self.validation.is_none() {
            let now = Instant::now();
            if now < self.next_frame_at {
                event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_frame_at));
                return;
            }
            self.next_frame_at = now + TARGET_FRAME_TIME;
            event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_frame_at));
        } else {
            event_loop.set_control_flow(ControlFlow::Poll);
        }

        if let Some(validation) = self.validation.as_mut() {
            validation.apply_script(&mut self.app);
        }
        self.app.tick();
        if self.app.take_audio_reset_requested() {
            self.audio.clear();
        }
        self.audio.push_from_app(&mut self.app);

        let frame = pixels.frame_mut();
        let source = self.app.frame_buffer();
        if frame.len() == source.len() {
            frame.copy_from_slice(source);
        }
        draw_menu_overlay(frame, self.app.current_slot(), self.app.paused());

        if let Some(validation) = self.validation.as_mut() {
            validation.record_frame(&self.app);
            if validation.should_exit() {
                self.app.request_exit();
            }
        } else if let Some(trace) = self.runtime_trace.as_mut() {
            trace.record(&self.app);
        }

        window.set_title(&self.app.window_title());
        if let Err(err) = pixels.render() {
            error!(%err, "render failed");
            event_loop.exit();
            return;
        }

        if self.app.should_exit() {
            event_loop.exit();
        } else {
            window.request_redraw();
        }
    }

    fn handle_keyboard(&mut self, event: &KeyEvent, event_loop: &ActiveEventLoop) {
        let pressed = event.state == ElementState::Pressed;
        if is_escape_key(event) && pressed {
            event_loop.exit();
            return;
        }

        if let Some(mapped) = map_key_event(event)
            && let Err(err) = self.app.handle_key(mapped, pressed)
        {
            warn!(%err, "keyboard action failed");
        }
    }

    fn handle_menu_click(&mut self, x: f64, y: f64, event_loop: &ActiveEventLoop) {
        match menu_command_at(x, y, self.config.scale) {
            Some(MenuCommand::Action(action)) => {
                if let Err(err) = self.app.handle_action(action) {
                    warn!(%err, "menu action failed");
                }
            }
            Some(MenuCommand::Reset) => self.app.reset(),
            Some(MenuCommand::Exit) => event_loop.exit(),
            None => {}
        }
    }
}

struct RuntimeTrace {
    file: File,
    frame: u64,
}

impl RuntimeTrace {
    fn new_for_current_app() -> Result<Self> {
        let Some(path) = runtime_trace_path() else {
            anyhow::bail!("runtime tracing is not enabled");
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut file = File::create(path)?;
        write_trace_header(&mut file)?;
        Ok(Self { file, frame: 0 })
    }

    fn record(&mut self, app: &App) {
        let debug = app.debug_state();
        let frame_hash = frame_hash(app.frame_buffer());
        let gameplay_hash = gameplay_hash(app.frame_buffer());
        let ram_digest = digest(&debug.cpu_ram);
        let oam_digest = digest(&debug.ppu.oam);
        if let Err(err) = write_trace_row(
            &mut self.file,
            self.frame,
            frame_hash,
            gameplay_hash,
            ram_digest,
            oam_digest,
            &debug,
        ) {
            warn!(%err, "failed to write runtime trace row");
        }
        if let Err(err) = self.file.flush() {
            warn!(%err, "failed to flush runtime trace");
        }
        self.frame += 1;
    }
}

fn runtime_trace_path() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("FC_EMU_RUNTIME_TRACE_PATH")
        && !path.is_empty()
    {
        return Some(PathBuf::from(path));
    }

    let exe = std::env::current_exe().ok()?;
    for ancestor in exe.ancestors() {
        if ancestor.extension().and_then(|ext| ext.to_str()) == Some("app") {
            return ancestor
                .parent()
                .map(|dist| dist.join("window-runtime-trace.csv"));
        }
    }

    None
}

impl ApplicationHandler for RuntimeApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if let Err(err) = self.create_window(event_loop) {
            error!(%err, "failed to resume app");
            event_loop.exit();
            return;
        }

        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(window) = self.window.as_ref() else {
            return;
        };
        if window.id() != window_id {
            return;
        }

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(pixels) = self.pixels.as_mut()
                    && let Err(err) = pixels.resize_surface(size.width.max(1), size.height.max(1))
                {
                    warn!(%err, "failed to resize pixel surface");
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if !event.repeat {
                    self.handle_keyboard(&event, event_loop);
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                LAST_CURSOR.with_borrow_mut(|cursor| *cursor = Some((position.x, position.y)));
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                ..
            } => {
                LAST_CURSOR.with_borrow(|cursor| {
                    if let Some((x, y)) = *cursor {
                        self.handle_menu_click(x, y, event_loop);
                    }
                });
            }
            WindowEvent::RedrawRequested => self.redraw(event_loop),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
        if self.validation.is_some() {
            self.redraw(event_loop);
        }
    }
}

struct ValidationState {
    config: WindowValidationConfig,
    frame: u64,
    last_hash: Option<u64>,
    last_gameplay_hash: Option<u64>,
    last_oam_digest: Option<u64>,
    stagnant_frames: u64,
    stagnant_gameplay_frames: u64,
    stagnant_oam_frames: u64,
    trace: Option<std::fs::File>,
    report: Option<WindowValidationReport>,
}

impl ValidationState {
    fn new(config: WindowValidationConfig) -> Self {
        let trace = config.trace_path.as_ref().and_then(|path| {
            if let Some(parent) = path.parent()
                && let Err(err) = std::fs::create_dir_all(parent)
            {
                warn!(%err, path = %path.display(), "failed to create validation trace directory");
                return None;
            }

            match std::fs::File::create(path) {
                Ok(mut file) => {
                    if let Err(err) = write_trace_header(&mut file) {
                        warn!(%err, path = %path.display(), "failed to write validation trace header");
                        return None;
                    }
                    Some(file)
                }
                Err(err) => {
                    warn!(%err, path = %path.display(), "failed to create validation trace");
                    None
                }
            }
        });

        Self {
            config,
            frame: 0,
            last_hash: None,
            last_gameplay_hash: None,
            last_oam_digest: None,
            stagnant_frames: 0,
            stagnant_gameplay_frames: 0,
            stagnant_oam_frames: 0,
            trace,
            report: None,
        }
    }

    fn apply_script(&self, app: &mut App) {
        set_button_at(app, self.frame, 90, Button::Start, true);
        set_button_at(app, self.frame, 98, Button::Start, false);
        set_button_at(app, self.frame, 170, Button::Right, true);
        set_button_at(app, self.frame, 170, Button::B, true);
        set_button_at(app, self.frame, 315, Button::A, true);
        set_button_at(app, self.frame, 344, Button::A, false);
        set_button_at(app, self.frame, 390, Button::A, true);
        set_button_at(app, self.frame, 418, Button::A, false);
        set_button_at(app, self.frame, 465, Button::A, true);
        set_button_at(app, self.frame, 492, Button::A, false);
        set_button_at(
            app,
            self.frame,
            self.config.max_frames + 1,
            Button::Right,
            false,
        );
        set_button_at(
            app,
            self.frame,
            self.config.max_frames + 1,
            Button::B,
            false,
        );
    }

    fn record_frame(&mut self, app: &App) {
        let hash = frame_hash(app.frame_buffer());
        let gameplay_hash = gameplay_hash(app.frame_buffer());
        let debug = app.debug_state();
        let ram_digest = digest(&debug.cpu_ram);
        let oam_digest = digest(&debug.ppu.oam);

        if self.last_hash == Some(hash) {
            self.stagnant_frames += 1;
        } else {
            self.stagnant_frames = 0;
            self.last_hash = Some(hash);
        }

        if self.last_gameplay_hash == Some(gameplay_hash) {
            self.stagnant_gameplay_frames += 1;
        } else {
            self.stagnant_gameplay_frames = 0;
            self.last_gameplay_hash = Some(gameplay_hash);
        }

        if self.last_oam_digest == Some(oam_digest) {
            self.stagnant_oam_frames += 1;
        } else {
            self.stagnant_oam_frames = 0;
            self.last_oam_digest = Some(oam_digest);
        }

        self.write_trace(hash, gameplay_hash, ram_digest, oam_digest, &debug);

        if debug.cpu.stopped {
            self.finish(self.report_for(
                false,
                "CPU stopped during window validation",
                ram_digest,
                oam_digest,
                &debug,
            ));
        } else if self.frame > 180 && self.stagnant_gameplay_frames >= self.config.stale_frame_limit
        {
            self.finish(self.report_for(
                false,
                "gameplay area stopped changing during scripted play",
                ram_digest,
                oam_digest,
                &debug,
            ));
        } else if self.frame > 180 && self.stagnant_oam_frames >= self.config.stale_frame_limit {
            self.finish(self.report_for(
                false,
                "OAM sprite data stopped changing during scripted play",
                ram_digest,
                oam_digest,
                &debug,
            ));
        } else if self.frame >= self.config.max_frames {
            self.finish(self.report_for(
                true,
                "scripted window validation completed",
                ram_digest,
                oam_digest,
                &debug,
            ));
        }

        self.frame += 1;
    }

    fn report_for(
        &self,
        passed: bool,
        reason: &str,
        ram_digest: u64,
        oam_digest: u64,
        debug: &crate::emulator::EmulatorDebugState,
    ) -> WindowValidationReport {
        WindowValidationReport {
            frames: self.frame,
            stagnant_frames: self.stagnant_frames,
            cpu_pc: debug.cpu.pc,
            cpu_cycles: debug.cpu.cycles,
            cpu_stopped: debug.cpu.stopped,
            ppu_status: debug.ppu.status,
            scroll_x: debug.ppu.scroll_x,
            scroll_y: debug.ppu.scroll_y,
            ram_digest,
            oam_digest,
            passed,
            reason: reason.to_string(),
        }
    }

    fn write_trace(
        &mut self,
        frame_hash: u64,
        gameplay_hash: u64,
        ram_digest: u64,
        oam_digest: u64,
        debug: &crate::emulator::EmulatorDebugState,
    ) {
        let Some(trace) = self.trace.as_mut() else {
            return;
        };

        if let Err(err) = write_trace_row(
            trace,
            self.frame,
            frame_hash,
            gameplay_hash,
            ram_digest,
            oam_digest,
            debug,
        ) {
            warn!(%err, "failed to write validation trace row");
        }
        if let Err(err) = trace.flush() {
            warn!(%err, "failed to flush validation trace");
        }
    }

    fn finish(&mut self, report: WindowValidationReport) {
        if self.report.is_none() {
            self.report = Some(report);
        }
    }

    fn should_exit(&self) -> bool {
        self.report.is_some()
    }
}

fn set_button_at(app: &mut App, frame: u64, target: u64, button: Button, pressed: bool) {
    if frame == target {
        app.set_button(button, pressed);
    }
}

fn write_trace_header(file: &mut File) -> std::io::Result<()> {
    writeln!(
        file,
        "frame,cpu_pc,cpu_a,cpu_x,cpu_y,cpu_sp,cpu_status,cpu_cycles,stopped,ppu_status,scanline,dot,scroll_x,scroll_y,vram_addr,temp_addr,frame_hash,gameplay_hash,ram_digest,oam_digest,oam0_y,oam0_tile,oam0_attr,oam0_x,ram_000e,ram_001d,ram_0086,ram_00ce,ram_006d,ram_075a"
    )
}

fn write_trace_row(
    file: &mut File,
    frame: u64,
    frame_hash: u64,
    gameplay_hash: u64,
    ram_digest: u64,
    oam_digest: u64,
    debug: &crate::emulator::EmulatorDebugState,
) -> std::io::Result<()> {
    let oam = &debug.ppu.oam;
    let ram = &debug.cpu_ram;
    writeln!(
        file,
        "{},{:04x},{:02x},{:02x},{:02x},{:02x},{:02x},{},{},{:02x},{},{},{},{},{:04x},{:04x},{:016x},{:016x},{:016x},{:016x},{:02x},{:02x},{:02x},{:02x},{:02x},{:02x},{:02x},{:02x},{:02x},{:02x}",
        frame,
        debug.cpu.pc,
        debug.cpu.a,
        debug.cpu.x,
        debug.cpu.y,
        debug.cpu.sp,
        debug.cpu.status,
        debug.cpu.cycles,
        debug.cpu.stopped,
        debug.ppu.status,
        debug.ppu.scanline,
        debug.ppu.dot,
        debug.ppu.scroll_x,
        debug.ppu.scroll_y,
        debug.ppu.vram_addr,
        debug.ppu.temp_addr,
        frame_hash,
        gameplay_hash,
        ram_digest,
        oam_digest,
        byte_at(oam, 0),
        byte_at(oam, 1),
        byte_at(oam, 2),
        byte_at(oam, 3),
        byte_at(ram, 0x000e),
        byte_at(ram, 0x001d),
        byte_at(ram, 0x0086),
        byte_at(ram, 0x00ce),
        byte_at(ram, 0x006d),
        byte_at(ram, 0x075a),
    )
}

fn frame_hash(frame: &[u8]) -> u64 {
    digest(frame)
}

fn gameplay_hash(frame: &[u8]) -> u64 {
    let start_y = 32usize;
    let end_y = NES_HEIGHT as usize;
    let row_len = NES_WIDTH as usize * 4;
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for y in start_y..end_y {
        let row_start = y * row_len;
        for byte in frame[row_start..row_start + row_len].iter().step_by(16) {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    hash
}

fn digest(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in bytes.iter().step_by(16) {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn byte_at(bytes: &[u8], index: usize) -> u8 {
    bytes.get(index).copied().unwrap_or(0)
}

thread_local! {
    static LAST_CURSOR: std::cell::RefCell<Option<(f64, f64)>> = const { std::cell::RefCell::new(None) };
}

fn map_key(key: KeyCode) -> Option<KeyboardKey> {
    match key {
        KeyCode::ArrowUp => Some(KeyboardKey::ArrowUp),
        KeyCode::ArrowDown => Some(KeyboardKey::ArrowDown),
        KeyCode::ArrowLeft => Some(KeyboardKey::ArrowLeft),
        KeyCode::ArrowRight => Some(KeyboardKey::ArrowRight),
        KeyCode::KeyX => Some(KeyboardKey::X),
        KeyCode::KeyZ => Some(KeyboardKey::Z),
        KeyCode::KeyS => Some(KeyboardKey::S),
        KeyCode::Enter | KeyCode::NumpadEnter => Some(KeyboardKey::Enter),
        KeyCode::ShiftRight => Some(KeyboardKey::RightShift),
        KeyCode::Tab => Some(KeyboardKey::Tab),
        KeyCode::Space => Some(KeyboardKey::Space),
        KeyCode::KeyP => Some(KeyboardKey::P),
        KeyCode::F5 => Some(KeyboardKey::F5),
        KeyCode::F9 => Some(KeyboardKey::F9),
        KeyCode::Digit1 => Some(KeyboardKey::Digit(1)),
        KeyCode::Digit2 => Some(KeyboardKey::Digit(2)),
        KeyCode::Digit3 => Some(KeyboardKey::Digit(3)),
        _ => None,
    }
}

fn map_key_event(event: &KeyEvent) -> Option<KeyboardKey> {
    if let PhysicalKey::Code(key) = event.physical_key
        && let Some(mapped) = map_key(key)
    {
        return Some(mapped);
    }

    match &event.logical_key {
        Key::Named(NamedKey::ArrowUp) => Some(KeyboardKey::ArrowUp),
        Key::Named(NamedKey::ArrowDown) => Some(KeyboardKey::ArrowDown),
        Key::Named(NamedKey::ArrowLeft) => Some(KeyboardKey::ArrowLeft),
        Key::Named(NamedKey::ArrowRight) => Some(KeyboardKey::ArrowRight),
        Key::Named(NamedKey::Enter) => Some(KeyboardKey::Enter),
        Key::Named(NamedKey::Space) => Some(KeyboardKey::Space),
        Key::Named(NamedKey::F5) => Some(KeyboardKey::F5),
        Key::Named(NamedKey::F9) => Some(KeyboardKey::F9),
        Key::Character(text) => map_character_key(text.as_str()),
        _ => None,
    }
}

fn map_character_key(text: &str) -> Option<KeyboardKey> {
    match text {
        "x" | "X" => Some(KeyboardKey::X),
        "z" | "Z" => Some(KeyboardKey::Z),
        "s" | "S" => Some(KeyboardKey::S),
        "p" | "P" => Some(KeyboardKey::P),
        "1" => Some(KeyboardKey::Digit(1)),
        "2" => Some(KeyboardKey::Digit(2)),
        "3" => Some(KeyboardKey::Digit(3)),
        _ => None,
    }
}

fn is_escape_key(event: &KeyEvent) -> bool {
    matches!(event.physical_key, PhysicalKey::Code(KeyCode::Escape))
        || matches!(event.logical_key, Key::Named(NamedKey::Escape))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MenuCommand {
    Action(AppControlAction),
    Reset,
    Exit,
}

fn menu_command_at(x: f64, y: f64, scale: u32) -> Option<MenuCommand> {
    let scale = f64::from(scale);
    if y > 24.0 * scale {
        return None;
    }

    let button = (x / (32.0 * scale)).floor() as u8;
    match button {
        0 => Some(MenuCommand::Action(AppControlAction::SaveState)),
        1 => Some(MenuCommand::Action(AppControlAction::LoadState)),
        2 => Some(MenuCommand::Reset),
        3 => Some(MenuCommand::Action(AppControlAction::TogglePause)),
        4 => Some(MenuCommand::Action(AppControlAction::SelectSaveSlot(
            SaveSlot::Slot(1),
        ))),
        5 => Some(MenuCommand::Action(AppControlAction::SelectSaveSlot(
            SaveSlot::Slot(2),
        ))),
        6 => Some(MenuCommand::Action(AppControlAction::SelectSaveSlot(
            SaveSlot::Slot(3),
        ))),
        7 => Some(MenuCommand::Exit),
        _ => None,
    }
}

struct AudioOutput {
    _stream: Option<Stream>,
    samples: Arc<Mutex<VecDeque<f32>>>,
}

impl AudioOutput {
    fn new() -> Self {
        match Self::try_new() {
            Ok(audio) => audio,
            Err(err) => {
                warn!(%err, "audio output disabled");
                Self {
                    _stream: None,
                    samples: Arc::new(Mutex::new(VecDeque::new())),
                }
            }
        }
    }

    fn try_new() -> Result<Self> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .context("no output audio device")?;
        let supported = device
            .default_output_config()
            .context("failed to query default output config")?;
        let sample_format = supported.sample_format();
        let config: StreamConfig = supported.into();
        let channels = usize::from(config.channels);
        let samples = Arc::new(Mutex::new(VecDeque::with_capacity(8192)));
        let stream_samples = samples.clone();
        let err_fn = |err| warn!(%err, "audio stream error");

        let stream = match sample_format {
            SampleFormat::F32 => device.build_output_stream(
                &config,
                move |data: &mut [f32], _| write_audio_f32(data, channels, &stream_samples),
                err_fn,
                None,
            ),
            other => {
                warn!(sample_format = %other, "unsupported sample format, outputting silence");
                device.build_output_stream(
                    &config,
                    move |data: &mut [f32], _| {
                        for sample in data {
                            *sample = 0.0;
                        }
                    },
                    err_fn,
                    None,
                )
            }
        }
        .context("failed to build output audio stream")?;
        stream
            .play()
            .context("failed to start output audio stream")?;

        info!(channels, "audio output started");
        Ok(Self {
            _stream: Some(stream),
            samples,
        })
    }

    fn push_from_app(&self, app: &mut App) {
        let mut drained = Vec::new();
        app.drain_audio_samples(&mut drained);
        if drained.is_empty() {
            return;
        }

        if let Ok(mut queue) = self.samples.lock() {
            let max_queue = 4096;
            while queue.len() > max_queue {
                queue.pop_front();
            }
            for sample in drained {
                if queue.len() >= max_queue {
                    queue.pop_front();
                }
                queue.push_back(sample.clamp(-1.0, 1.0));
            }
        }
    }

    fn clear(&self) {
        if let Ok(mut queue) = self.samples.lock() {
            queue.clear();
        }
    }
}

fn write_audio_f32(data: &mut [f32], channels: usize, samples: &Arc<Mutex<VecDeque<f32>>>) {
    let mut queue = samples.lock().ok();
    for frame in data.chunks_mut(channels) {
        let sample = queue
            .as_mut()
            .and_then(|queue| queue.pop_front())
            .unwrap_or(0.0);
        for output in frame {
            *output = sample;
        }
    }
}

fn draw_menu_overlay(frame: &mut [u8], current_slot: u8, paused: bool) {
    const MENU_HEIGHT: usize = 24;
    const BUTTON_WIDTH: usize = 32;
    let width = NES_WIDTH as usize;

    for y in 0..MENU_HEIGHT {
        for x in 0..width {
            let i = (y * width + x) * 4;
            frame[i] = 18;
            frame[i + 1] = 22;
            frame[i + 2] = 28;
            frame[i + 3] = 255;
        }
    }

    let colors = [
        [54, 96, 168, 255],
        [58, 128, 84, 255],
        [150, 98, 44, 255],
        if paused {
            [174, 58, 58, 255]
        } else {
            [92, 92, 112, 255]
        },
        slot_color(current_slot, 1),
        slot_color(current_slot, 2),
        slot_color(current_slot, 3),
        [108, 62, 136, 255],
    ];

    for (button, color) in colors.iter().enumerate() {
        let x0 = button * BUTTON_WIDTH;
        let x1 = ((button + 1) * BUTTON_WIDTH).min(width);
        for y in 3..(MENU_HEIGHT - 3) {
            for x in (x0 + 2)..x1.saturating_sub(2) {
                let i = (y * width + x) * 4;
                frame[i..i + 4].copy_from_slice(color);
            }
        }
    }

    let labels = [
        "SAV",
        "LOD",
        "RST",
        if paused { "RUN" } else { "PAU" },
        "1",
        "2",
        "3",
        "X",
    ];
    for (button, label) in labels.iter().enumerate() {
        draw_menu_label(frame, button * BUTTON_WIDTH, BUTTON_WIDTH, label);
    }
}

fn slot_color(current_slot: u8, slot: u8) -> [u8; 4] {
    if current_slot == slot {
        [200, 184, 64, 255]
    } else {
        [86, 92, 104, 255]
    }
}

fn draw_menu_label(frame: &mut [u8], button_x: usize, button_width: usize, text: &str) {
    const SCALE: usize = 2;
    const GLYPH_WIDTH: usize = 3;
    const SPACING: usize = 1;

    let glyphs = text.chars().count();
    let text_width = if glyphs == 0 {
        0
    } else {
        (glyphs * GLYPH_WIDTH + (glyphs - 1) * SPACING) * SCALE
    };
    let mut x = button_x + button_width.saturating_sub(text_width) / 2;
    let y = 7;

    for ch in text.chars() {
        draw_glyph(frame, x, y, SCALE, ch);
        x += (GLYPH_WIDTH + SPACING) * SCALE;
    }
}

fn draw_glyph(frame: &mut [u8], x: usize, y: usize, scale: usize, ch: char) {
    let Some(rows) = glyph_rows(ch) else {
        return;
    };
    let width = NES_WIDTH as usize;
    let color = [244, 244, 236, 255];

    for (row, bits) in rows.iter().enumerate() {
        for col in 0..3 {
            if bits & (1 << (2 - col)) == 0 {
                continue;
            }
            for dy in 0..scale {
                for dx in 0..scale {
                    let px = x + col * scale + dx;
                    let py = y + row * scale + dy;
                    let i = (py * width + px) * 4;
                    frame[i..i + 4].copy_from_slice(&color);
                }
            }
        }
    }
}

fn glyph_rows(ch: char) -> Option<[u8; 5]> {
    match ch {
        '1' => Some([0b010, 0b110, 0b010, 0b010, 0b111]),
        '2' => Some([0b111, 0b001, 0b111, 0b100, 0b111]),
        '3' => Some([0b111, 0b001, 0b111, 0b001, 0b111]),
        'A' => Some([0b010, 0b101, 0b111, 0b101, 0b101]),
        'D' => Some([0b110, 0b101, 0b101, 0b101, 0b110]),
        'L' => Some([0b100, 0b100, 0b100, 0b100, 0b111]),
        'O' => Some([0b111, 0b101, 0b101, 0b101, 0b111]),
        'P' => Some([0b110, 0b101, 0b110, 0b100, 0b100]),
        'R' => Some([0b110, 0b101, 0b110, 0b101, 0b101]),
        'S' => Some([0b111, 0b100, 0b111, 0b001, 0b111]),
        'T' => Some([0b111, 0b010, 0b010, 0b010, 0b010]),
        'U' => Some([0b101, 0b101, 0b101, 0b101, 0b111]),
        'V' => Some([0b101, 0b101, 0b101, 0b101, 0b010]),
        'X' => Some([0b101, 0b101, 0b010, 0b101, 0b101]),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_window_size_is_three_x_nes_resolution() {
        assert_eq!(WindowConfig::default().content_size(), (768, 720));
    }

    #[test]
    fn maps_winit_keys_to_project_keys() {
        assert_eq!(map_key(KeyCode::KeyX), Some(KeyboardKey::X));
        assert_eq!(map_key(KeyCode::KeyS), Some(KeyboardKey::S));
        assert_eq!(map_key(KeyCode::F5), Some(KeyboardKey::F5));
        assert_eq!(map_key(KeyCode::Digit3), Some(KeyboardKey::Digit(3)));
        assert_eq!(map_key(KeyCode::Escape), None);
    }

    #[test]
    fn maps_menu_coordinates_to_commands() {
        let scale = DEFAULT_SCALE;
        let scaled = |x| x * f64::from(scale);

        assert_eq!(
            menu_command_at(scaled(8.0), 8.0, scale),
            Some(MenuCommand::Action(AppControlAction::SaveState))
        );
        assert_eq!(
            menu_command_at(scaled(40.0), 8.0, scale),
            Some(MenuCommand::Action(AppControlAction::LoadState))
        );
        assert_eq!(
            menu_command_at(scaled(72.0), 8.0, scale),
            Some(MenuCommand::Reset)
        );
        assert_eq!(
            menu_command_at(scaled(136.0), 8.0, scale),
            Some(MenuCommand::Action(AppControlAction::SelectSaveSlot(
                SaveSlot::Slot(1),
            )))
        );
        assert_eq!(
            menu_command_at(scaled(232.0), 8.0, scale),
            Some(MenuCommand::Exit)
        );
        assert_eq!(menu_command_at(8.0, scaled(25.0), scale), None);
    }
}
