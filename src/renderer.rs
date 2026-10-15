// Выбор способа отрисовки. Если в системе есть GPU, всегда используется GPU;
// только если ни один способ отрисовки на GPU не запустился, приложение
// переходит на отрисовку на CPU (программную).
//
// Способы перебираются по порядку, пока один из них не запустится:
//   1. GPU через OpenGL (обязательно аппаратное ускорение);
//   2. GPU через wgpu (Direct3D 12 / Vulkan) — если для GPU нет драйвера OpenGL;
//   3. CPU через wgpu (WARP в Windows, lavapipe/llvmpipe в Linux);
//   4. CPU через OpenGL (llvmpipe из Mesa в Linux).
// Код одинаков для Windows и Linux: доступные на конкретной системе
// реализации выбирают сами библиотеки.
use eframe::egui_wgpu::{self, wgpu};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

// Способ отрисовки, который запускается сейчас (индекс в Attempt::ALL).
static CURRENT: AtomicU8 = AtomicU8::new(0);
// Способ «GPU» запустился, но оказался программным — окно закрыто, нужен следующий.
static SOFTWARE_GPU_REJECTED: AtomicBool = AtomicBool::new(false);

// Переменная окружения, которой можно принудительно выбрать способ отрисовки:
// "gpu" — только GPU, "cpu" — только CPU. Нужна для диагностики проблем с драйверами.
const RENDERER_ENV: &str = "PYATORKA_RENDERER";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Attempt {
    GpuOpenGl,
    GpuWgpu,
    CpuWgpu,
    CpuOpenGl,
}

impl Attempt {
    const ALL: [Attempt; 4] = [
        Attempt::GpuOpenGl,
        Attempt::GpuWgpu,
        Attempt::CpuWgpu,
        Attempt::CpuOpenGl,
    ];

    fn uses_gpu(self) -> bool {
        matches!(self, Attempt::GpuOpenGl | Attempt::GpuWgpu)
    }

    // Описание для журнала.
    pub fn describe(self) -> &'static str {
        match self {
            Attempt::GpuOpenGl => "GPU / OpenGL",
            Attempt::GpuWgpu => "GPU / wgpu",
            Attempt::CpuWgpu => "CPU / wgpu",
            Attempt::CpuOpenGl => "CPU / OpenGL",
        }
    }
}

// Порядок попыток с учётом переменной окружения PYATORKA_RENDERER.
pub fn attempts() -> Vec<Attempt> {
    let forced = std::env::var(RENDERER_ENV)
        .map(|v| v.trim().to_ascii_lowercase())
        .unwrap_or_default();
    Attempt::ALL
        .into_iter()
        .filter(|attempt| match forced.as_str() {
            "gpu" => attempt.uses_gpu(),
            "cpu" => !attempt.uses_gpu(),
            _ => true,
        })
        .collect()
}

// Запоминает, какой способ отрисовки сейчас запускается.
pub fn set_current(attempt: Attempt) {
    let index = Attempt::ALL.iter().position(|a| *a == attempt).unwrap_or(0);
    CURRENT.store(index as u8, Ordering::SeqCst);
    SOFTWARE_GPU_REJECTED.store(false, Ordering::SeqCst);
}

// Вызывается, когда окно фактически рисуется на CPU. Если при этом запускался
// способ «GPU» (OpenGL иногда сам переходит на программную реализацию, например
// llvmpipe), результат отклоняется — тогда возвращается true, и окно нужно закрыть,
// чтобы попробовать GPU через wgpu и лишь затем перейти на CPU.
pub fn reject_if_software_gpu() -> bool {
    let current = Attempt::ALL[CURRENT.load(Ordering::SeqCst) as usize];
    let reject = current.uses_gpu();
    if reject {
        SOFTWARE_GPU_REJECTED.store(true, Ordering::SeqCst);
    }
    reject
}

// Был ли последний запуск отклонён как программный вместо GPU (признак сбрасывается).
pub fn take_software_gpu_rejection() -> bool {
    SOFTWARE_GPU_REJECTED.swap(false, Ordering::SeqCst)
}

// Является ли ошибка запуска ошибкой инициализации отрисовки — тогда стоит
// попробовать следующий способ. Прочие ошибки сменой отрисовки не исправить.
pub fn is_renderer_error(error: &eframe::Error) -> bool {
    matches!(
        error,
        eframe::Error::Glutin(_)
            | eframe::Error::NoGlutinConfigs(..)
            | eframe::Error::OpenGL(_)
            | eframe::Error::Wgpu(_)
    )
}

// Настраивает параметры запуска eframe под выбранный способ отрисовки.
pub fn apply(attempt: Attempt, options: &mut eframe::NativeOptions) {
    match attempt {
        Attempt::GpuOpenGl => {
            options.renderer = eframe::Renderer::Glow;
            options.glow_options.hardware_acceleration =
                eframe::egui_glow::HardwareAcceleration::Required;
        }
        Attempt::CpuOpenGl => {
            options.renderer = eframe::Renderer::Glow;
            options.glow_options.hardware_acceleration =
                eframe::egui_glow::HardwareAcceleration::Off;
        }
        Attempt::GpuWgpu => {
            options.renderer = eframe::Renderer::Wgpu;
            options.wgpu_options.wgpu_setup = wgpu_setup(false);
        }
        Attempt::CpuWgpu => {
            options.renderer = eframe::Renderer::Wgpu;
            options.wgpu_options.wgpu_setup = wgpu_setup(true);
        }
    }
}

// Насколько адаптер подходит для отрисовки на GPU: чем больше число, тем лучше.
fn gpu_rank(device_type: wgpu::DeviceType) -> Option<u8> {
    match device_type {
        wgpu::DeviceType::DiscreteGpu => Some(3),
        wgpu::DeviceType::IntegratedGpu => Some(2),
        wgpu::DeviceType::VirtualGpu => Some(1),
        wgpu::DeviceType::Other => Some(0),
        wgpu::DeviceType::Cpu => None,
    }
}

// Настройка wgpu: `cpu` выбирает программный адаптер, иначе — лучший GPU.
fn wgpu_setup(cpu: bool) -> egui_wgpu::WgpuSetup {
    let mut setup = egui_wgpu::WgpuSetupCreateNew::without_display_handle();
    // Окно прозрачное (скруглённые углы рисует само приложение). В Direct3D 12
    // прозрачность поддерживает только вывод через DirectComposition.
    setup
        .instance_descriptor
        .backend_options
        .dx12
        .presentation_system = wgpu::Dx12SwapchainKind::DxgiFromVisual;
    setup.native_adapter_selector = Some(Arc::new(move |adapters, surface| {
        let usable = adapters
            .iter()
            .filter(|adapter| surface.is_none_or(|surface| adapter.is_surface_supported(surface)));
        let chosen = if cpu {
            usable
                .filter(|adapter| adapter.get_info().device_type == wgpu::DeviceType::Cpu)
                .min_by_key(|adapter| adapter.get_info().backend == wgpu::Backend::Gl)
        } else {
            usable
                .filter_map(|adapter| {
                    gpu_rank(adapter.get_info().device_type).map(|rank| (rank, adapter))
                })
                .max_by_key(|(rank, _)| *rank)
                .map(|(_, adapter)| adapter)
        };
        chosen.cloned().ok_or_else(|| {
            if cpu {
                "программный адаптер wgpu не найден".to_string()
            } else {
                "GPU-адаптер wgpu не найден".to_string()
            }
        })
    }));
    egui_wgpu::WgpuSetup::CreateNew(setup)
}

// Что фактически рисует окно: GPU или CPU, устройство и графический интерфейс.
pub struct RenderInfo {
    pub on_cpu: bool,
    pub device: String,
    pub api: String,
}

// Признаки программных (CPU) реализаций OpenGL в строке GL_RENDERER.
fn is_software_gl(renderer: &str) -> bool {
    let renderer = renderer.to_ascii_lowercase();
    [
        "llvmpipe",
        "softpipe",
        "swrast",
        "software",
        "gdi generic",
        "swiftshader",
        "basic render",
        "warp",
    ]
    .iter()
    .any(|marker| renderer.contains(marker))
}

// Определяет фактический способ отрисовки по контексту уже созданного окна.
pub fn render_info(frame: &eframe::Frame) -> Option<RenderInfo> {
    if let Some(gl) = frame.gl() {
        use eframe::glow::HasContext as _;
        // SAFETY: запрос строки состояния у действующего контекста OpenGL,
        // который eframe делает текущим перед вызовом кода приложения.
        let device = unsafe { gl.get_parameter_string(eframe::glow::RENDERER) };
        return Some(RenderInfo {
            on_cpu: is_software_gl(&device),
            device,
            api: "OpenGL".to_string(),
        });
    }
    if let Some(state) = frame.wgpu_render_state() {
        let info = state.adapter.get_info();
        return Some(RenderInfo {
            on_cpu: info.device_type == wgpu::DeviceType::Cpu,
            device: info.name,
            api: format!("{:?}", info.backend),
        });
    }
    None
}
