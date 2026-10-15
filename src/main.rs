// В релизной сборке под Windows не открывать окно консоли. Атрибут может стоять
// только в корне крейта, поэтому он здесь, а не в platform.rs; на других
// системах он ни на что не влияет.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// Точка входа приложения Пятёрка: окно, состояние, отрисовка и обработчики UI.
// Код одинаков для Windows и Linux; всё, что зависит от ОС, — в модуле platform.
mod diff;
mod encoding;
mod file_picker;
mod geometry;
mod i18n;
mod platform;
mod renderer;
mod startup;
mod style;

use eframe::egui;
use egui::{Color32, Pos2, Rect, Stroke, Vec2};
use geometry::{Geometry, TextStyle};
use i18n::{Lang, t};

// Рисует текст заданного пиксельного размера и возвращает его ограничивающий прямоугольник
// (нужен вызывающему коду для последующей отрисовки подчёркивания/зачёркивания).
fn draw_scaled_text(
    _ui: &egui::Ui,
    painter: &egui::Painter,
    anchor: Pos2,
    align: egui::Align2,
    text: &str,
    px_size: f32,
    col: Color32,
) -> Rect {
    let size = px_size.round().max(1.0);
    painter.text(anchor, align, text, egui::FontId::proportional(size), col)
}

// Рисует окружность обводкой, центрированной по радиусу (egui рисует обводку
// окружности снаружи радиуса). Если обводка шире диаметра, получается сплошной круг.
fn circle_stroke_centered(painter: &egui::Painter, center: Pos2, radius: f32, stroke: Stroke) {
    let half = stroke.width / 2.0;
    if radius > half {
        painter.circle_stroke(center, radius - half, stroke);
    } else {
        painter.circle_filled(center, radius + half, stroke.color);
    }
}

// Рисует поверх текста подчёркивание/надчёркивание/зачёркивание согласно TextStyle.
fn draw_text_decorations(
    painter: &egui::Painter,
    text_rect: Rect,
    style: &TextStyle,
    col: Color32,
) {
    if !style.underline && !style.overline && !style.strikethrough {
        return;
    }

    let stroke_w = (text_rect.height() * 0.06).max(0.6);
    let stroke = Stroke::new(stroke_w, col);
    if style.underline {
        let y = text_rect.bottom() + stroke_w;
        painter.line_segment(
            [
                Pos2::new(text_rect.left(), y),
                Pos2::new(text_rect.right(), y),
            ],
            stroke,
        );
    }
    if style.overline {
        let y = text_rect.top() - stroke_w;
        painter.line_segment(
            [
                Pos2::new(text_rect.left(), y),
                Pos2::new(text_rect.right(), y),
            ],
            stroke,
        );
    }
    if style.strikethrough {
        let y = (text_rect.top() + text_rect.bottom()) / 2.0;
        painter.line_segment(
            [
                Pos2::new(text_rect.left(), y),
                Pos2::new(text_rect.right(), y),
            ],
            stroke,
        );
    }
}

// Формирует подпись варианта кодировки в диалоге выбора, например "Windows-1251 (кириллица)".
fn encoding_choice_label(lang: Lang, index: usize) -> String {
    let (code_name, _enc, desc_key) = geometry::ENCODING_CHOICES[index];
    if desc_key.is_empty() {
        code_name.to_string()
    } else {
        format!("{} ({})", code_name, t(lang, desc_key))
    }
}

// Настройки, сохраняемые между запусками приложения (язык, цвета, прозрачность).
#[derive(serde::Serialize, serde::Deserialize)]
struct Settings {
    lang: Lang,
    color_a: [u8; 3],
    opacity_a: f32,
    color_b: [u8; 3],
    opacity_b: f32,

    #[serde(default = "default_diff_color_a_arr")]
    diff_color_a: [u8; 3],

    #[serde(default = "default_diff_color_b_arr")]
    diff_color_b: [u8; 3],
}

// Значения по умолчанию для serde, если в сохранённых настройках ещё нет полей diff_color_*
// (совместимость со старыми файлами настроек, сохранёнными до появления подсветки различий).
fn default_diff_color_a_arr() -> [u8; 3] {
    [
        style::DIFF_COLOR_A_DEFAULT.r(),
        style::DIFF_COLOR_A_DEFAULT.g(),
        style::DIFF_COLOR_A_DEFAULT.b(),
    ]
}

fn default_diff_color_b_arr() -> [u8; 3] {
    [
        style::DIFF_COLOR_B_DEFAULT.r(),
        style::DIFF_COLOR_B_DEFAULT.g(),
        style::DIFF_COLOR_B_DEFAULT.b(),
    ]
}

// Ключ, под которым настройки хранятся в eframe::Storage.
const SETTINGS_KEY: &str = "pyatorka_settings";

// Состояние одного загруженного файла (слот A или слот B).
struct FileSlot {
    path: Option<String>,
    geo: Option<Geometry>,
    color: Color32,
    visible: bool,
    opacity: f32,
    scale: f32,
    r#gen: u64,
    encoding_choice: Option<usize>,
}

impl FileSlot {
    fn new(color: Color32) -> Self {
        Self {
            path: None,
            geo: None,
            color,
            visible: true,
            opacity: 1.0,
            scale: 1.0,
            r#gen: 0,
            encoding_choice: None,
        }
    }
}

// Границы масштаба просмотра, чтобы колесо мыши/пинч не уводили вид в бесконечность.
const ZOOM_MIN: f32 = 1.0e-4;
const ZOOM_MAX: f32 = 1.0e6;

// Главное состояние приложения (реализует eframe::App).
struct App {
    lang: Lang,
    slot_a: FileSlot,
    slot_b: FileSlot,
    pan: Vec2,
    zoom: f32,
    error: Option<LoadError>,
    needs_fit: bool,
    last_canvas_size: Option<Vec2>,
    center_offset: Pos2,
    show_license: bool,
    diff_enabled: bool,
    diff_a_visible: bool,
    diff_b_visible: bool,
    diff_color_a: Color32,
    diff_color_b: Color32,
    diff_cache: Option<diff::DiffResult>,
    diff_cache_key: Option<(u32, u32, u64, u64)>,
    pending_encoding: Option<PendingEncoding>,
    // Встроенный обзор файлов, если системный диалог выбора недоступен.
    file_picker: Option<file_picker::FilePicker>,
    scale_slider_dragging: bool,
    subtitle_width_cache: Option<(Lang, f32)>,
    window_configured: bool,
    // Фактический способ отрисовки (GPU или CPU), определяется в первом кадре.
    render_info: Option<renderer::RenderInfo>,
}

// Ошибка загрузки файла, показываемая в модальном окне.
struct LoadError {
    file_name: String,
    message: String,
}

// Файл, ожидающий от пользователя ручного выбора кодировки текста.
struct PendingEncoding {
    path: std::path::PathBuf,
    slot_index: usize,
    selected: usize,
}

impl Default for App {
    fn default() -> Self {
        Self {
            lang: Lang::default(),
            slot_a: FileSlot::new(style::COLOR_A_DEFAULT),
            slot_b: FileSlot::new(style::COLOR_B_DEFAULT),
            pan: Vec2::ZERO,
            zoom: 1.0,
            error: None,
            needs_fit: false,
            last_canvas_size: None,
            center_offset: Pos2::ZERO,
            show_license: false,
            diff_enabled: false,
            diff_a_visible: true,
            diff_b_visible: true,
            diff_color_a: style::DIFF_COLOR_A_DEFAULT,
            diff_color_b: style::DIFF_COLOR_B_DEFAULT,
            diff_cache: None,
            diff_cache_key: None,
            pending_encoding: None,
            file_picker: None,
            scale_slider_dragging: false,
            subtitle_width_cache: None,
            window_configured: false,
            render_info: None,
        }
    }
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        style::apply(&cc.egui_ctx);
        // Шаг прокрутки колесом мыши (для списков и панелей) — как у холста.
        cc.egui_ctx
            .options_mut(|o| o.input_options.line_scroll_speed = POINTS_PER_SCROLL_LINE);
        let mut app = App::default();

        if let Some(storage) = cc.storage {
            if let Some(s) = eframe::get_value::<Settings>(storage, SETTINGS_KEY) {
                app.apply_settings(s);
            }
        }
        app
    }

    // Собирает текущее состояние UI в сериализуемую структуру Settings.
    fn to_settings(&self) -> Settings {
        let color_a = [
            self.slot_a.color.r(),
            self.slot_a.color.g(),
            self.slot_a.color.b(),
        ];
        let color_b = [
            self.slot_b.color.r(),
            self.slot_b.color.g(),
            self.slot_b.color.b(),
        ];
        let diff_color_a = [
            self.diff_color_a.r(),
            self.diff_color_a.g(),
            self.diff_color_a.b(),
        ];
        let diff_color_b = [
            self.diff_color_b.r(),
            self.diff_color_b.g(),
            self.diff_color_b.b(),
        ];
        Settings {
            lang: self.lang,
            color_a,
            opacity_a: self.slot_a.opacity,
            color_b,
            opacity_b: self.slot_b.opacity,
            diff_color_a,
            diff_color_b,
        }
    }

    // Восстанавливает состояние UI из сохранённых настроек.
    fn apply_settings(&mut self, s: Settings) {
        self.lang = s.lang;
        self.slot_a.color = Color32::from_rgb(s.color_a[0], s.color_a[1], s.color_a[2]);
        self.slot_a.opacity = s.opacity_a;
        self.slot_b.color = Color32::from_rgb(s.color_b[0], s.color_b[1], s.color_b[2]);
        self.slot_b.opacity = s.opacity_b;
        self.diff_color_a =
            Color32::from_rgb(s.diff_color_a[0], s.diff_color_a[1], s.diff_color_a[2]);
        self.diff_color_b =
            Color32::from_rgb(s.diff_color_b[0], s.diff_color_b[1], s.diff_color_b[2]);
    }

    // Пересчитывает кэш различий, только если изменился масштаб или содержимое
    // одного из файлов (ключ кэша), и только когда оба файла загружены.
    // Во время перетаскивания (drag) пересчёт откладывается, чтобы не тормозить UI.
    fn update_diff_cache(&mut self, ctx: &egui::Context) {
        if !self.diff_enabled {
            return;
        }
        match (&self.slot_a.geo, &self.slot_b.geo) {
            (Some(_), Some(_)) => {
                let key = (
                    self.slot_a.scale.to_bits(),
                    self.slot_b.scale.to_bits(),
                    self.slot_a.r#gen,
                    self.slot_b.r#gen,
                );
                if self.diff_cache_key != Some(key) {
                    let dragging = ctx.input(|i| i.pointer.any_down());
                    if dragging {
                        ctx.request_repaint();
                        return;
                    }
                    let ga = self.slot_a.geo.as_ref().unwrap();
                    let gb = self.slot_b.geo.as_ref().unwrap();
                    self.diff_cache = Some(diff::compute_diff(
                        ga,
                        self.slot_a.scale,
                        gb,
                        self.slot_b.scale,
                    ));
                    self.diff_cache_key = Some(key);
                }
            }
            _ => {
                self.diff_cache = None;
                self.diff_cache_key = None;
            }
        }
    }

    // Открывает системный диалог выбора DXF-файла и загружает выбранный файл в слот.
    // Если системный диалог недоступен, открывается встроенный обзор файлов.
    fn open_dialog(
        &mut self,
        slot_index: usize,
        forced_encoding: Option<&'static encoding_rs::Encoding>,
    ) {
        file_picker::reset_native_dialog_failure();
        let picked = rfd::FileDialog::new()
            .add_filter("DXF", &["dxf", "DXF"])
            .set_title(t(self.lang, "select_file_title"))
            .pick_file();
        match picked {
            Some(path) => self.load_path_into_slot(path, slot_index, forced_encoding),
            None if file_picker::native_dialog_failed() => {
                self.file_picker = Some(file_picker::FilePicker::new(
                    slot_index,
                    forced_encoding,
                    self.picker_start_dir(slot_index),
                ));
            }
            None => {}
        }
    }

    // Папка, с которой начинается встроенный обзор файлов: папка уже открытого
    // в этом слоте файла, иначе — папка файла из другого слота.
    fn picker_start_dir(&self, slot_index: usize) -> Option<std::path::PathBuf> {
        let (this, other) = if slot_index == 0 {
            (&self.slot_a, &self.slot_b)
        } else {
            (&self.slot_b, &self.slot_a)
        };
        [this, other].into_iter().find_map(|slot| {
            slot.path
                .as_deref()
                .and_then(|p| std::path::Path::new(p).parent())
                .map(|p| p.to_path_buf())
        })
    }

    // Загружает DXF-файл по пути в указанный слот (A или B), обновляя геометрию,
    // счётчик поколений (gen) для кэша различий и, при необходимости, запрашивая
    // у пользователя кодировку текста.
    fn load_path_into_slot(
        &mut self,
        path: std::path::PathBuf,
        slot_index: usize,
        forced_encoding: Option<&'static encoding_rs::Encoding>,
    ) {
        match geometry::load_dxf(&path, forced_encoding) {
            Ok(geometry::DxfLoadOutcome::Loaded(geo)) => {
                let slot = if slot_index == 0 {
                    &mut self.slot_a
                } else {
                    &mut self.slot_b
                };
                slot.path = Some(path.display().to_string());
                slot.geo = Some(geo);
                slot.r#gen = slot.r#gen.wrapping_add(1);
                self.error = None;
                self.pending_encoding = None;
                self.needs_fit = true;
            }
            Ok(geometry::DxfLoadOutcome::NeedsEncoding { guess_index }) => {
                self.pending_encoding = Some(PendingEncoding {
                    path,
                    slot_index,
                    selected: guess_index.unwrap_or(0),
                });
                self.error = None;
            }
            Err(e) => {
                let file_name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.display().to_string());
                self.error = Some(LoadError {
                    file_name,
                    message: e.to_string(),
                });
                self.pending_encoding = None;
            }
        }
    }

    // Общий охватывающий прямоугольник обоих загруженных файлов
    // (с учётом индивидуального масштаба каждого слота).
    fn combined_bounds(&self) -> Option<Rect> {
        let mut result: Option<Rect> = None;
        for slot in [&self.slot_a, &self.slot_b] {
            if let Some(g) = &slot.geo {
                let s = slot.scale;
                let scaled = Rect::from_min_max(
                    Pos2::new(g.bounds.min.x * s, g.bounds.min.y * s),
                    Pos2::new(g.bounds.max.x * s, g.bounds.max.y * s),
                );
                result = Some(match result {
                    None => scaled,
                    Some(r) => r.union(scaled),
                });
            }
        }
        result
    }

    // Подбирает масштаб и сброс панорамирования так, чтобы вся геометрия
    // поместилась в видимую область холста с небольшим отступом (margin).
    fn fit_view(&mut self, canvas_size: Vec2) {
        if let Some(b) = self.combined_bounds() {
            let w = (b.width()).max(1.0);
            let h = (b.height()).max(1.0);
            let margin = 0.9;
            let scale_x = canvas_size.x * margin / w;
            let scale_y = canvas_size.y * margin / h;
            self.zoom = scale_x.min(scale_y).clamp(ZOOM_MIN, ZOOM_MAX);

            self.pan = Vec2::ZERO;
            self.center_offset = b.center();
        } else {
            self.zoom = 1.0;
            self.pan = Vec2::ZERO;
        }
        self.needs_fit = false;
    }
}

impl eframe::App for App {
    // Прозрачный фон окна нужен для собственного скругления углов окна без рамки.
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        egui::Color32::TRANSPARENT.to_normalized_gamma_f32()
    }

    // Сохраняет настройки приложения при закрытии/потере фокуса.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        let s = self.to_settings();
        eframe::set_value(storage, SETTINGS_KEY, &s);
    }

    // Состояние виджетов egui (раскрытые панели и т.п.) не сохраняем между запусками.
    fn persist_egui_memory(&self) -> bool {
        false
    }

    // Основной цикл отрисовки кадра: заголовок, панель управления, холст с чертежами,
    // модальные окна (лицензия, ошибка загрузки, выбор кодировки).
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = &ui.ctx().clone();
        self.scale_slider_dragging = false;

        if !self.window_configured {
            // Отмечается сразу: следующий кадр (в том числе до закрытия окна,
            // отклонённого ниже) не должен повторять первичную настройку.
            self.window_configured = true;
            startup::renderer_started();
            platform::configure_window(frame);
            self.render_info = renderer::render_info(frame);
            if let Some(info) = &self.render_info {
                let kind = if info.on_cpu { "CPU" } else { "GPU" };
                eprintln!("Отрисовка: {kind} — {} ({})", info.device, info.api);
                // Способ «GPU» оказался программным (например, OpenGL через llvmpipe):
                // окно сразу скрывается и закрывается, и main пробует GPU через wgpu.
                if info.on_cpu && renderer::reject_if_software_gpu() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    return;
                }
            }
        }

        if !self.modal_open() {
            handle_resize_edges(ctx);
        }

        self.custom_title_bar(ui);

        self.license_window(ctx);
        self.error_window(ctx);
        self.file_picker_window(ctx);
        self.encoding_choice_window(ctx);

        self.update_diff_cache(ctx);

        // Ширина содержимого левой панели управления фиксирована. Если какая-то строка
        // (например, при длинных подписях) не помещается, панель расширяется под неё.
        const PANEL_FIXED_WIDTH: f32 = 280.0;
        // Размер Panel задаётся вместе с отступами рамки (14 px с каждой стороны).
        let panel_width = PANEL_FIXED_WIDTH + 14.0 * 2.0;

        let side_frame = egui::Frame::new()
            .fill(Color32::from_rgb(246, 246, 248))
            // Обводка (1 px) в egui входит в размер рамки, поэтому внутренний
            // отступ уменьшен на её толщину — содержимое остаётся на прежнем месте.
            .inner_margin(egui::Margin::same(13))
            .corner_radius(egui::CornerRadius {
                nw: 0,
                ne: 0,
                sw: 14,
                se: 0,
            })
            .stroke(Stroke::new(1.0, Color32::from_rgb(228, 228, 232)));

        let side_panel_id = egui::Id::new("control_panel");
        let size_before = panel_outer_rect(ctx, side_panel_id);
        let side_panel = egui::Panel::left(side_panel_id)
            .resizable(false)
            .show_separator_line(false)
            .min_size(panel_width)
            .default_size(panel_width)
            .frame(side_frame)
            .show(ui, |ui| {
                ui.set_max_width(PANEL_FIXED_WIDTH);
                self.control_panel_body(ui);
            });
        paint_panel_separator(ui, side_panel.response.rect, PanelEdge::Right);
        discard_if_panel_resized(ctx, side_panel_id, size_before, 0);

        let central_frame = egui::Frame::new()
            .fill(Color32::from_rgb(252, 252, 253))
            .corner_radius(egui::CornerRadius {
                nw: 0,
                ne: 0,
                sw: 0,
                se: 14,
            })
            .stroke(Stroke::new(1.0, Color32::from_rgb(228, 228, 232)));
        egui::CentralPanel::default()
            .frame(central_frame)
            .show(ui, |ui| {
                let rect = ui.available_rect_before_wrap();

                // При первом кадре (или после сброса) подгоняем масштаб под холст;
                // при изменении размера окна пропорционально подстраиваем zoom/pan,
                // чтобы содержимое визуально не "прыгало".
                if self.needs_fit {
                    self.fit_view(rect.size());
                    self.last_canvas_size = Some(rect.size());
                } else if let Some(prev_size) = self.last_canvas_size {
                    let cur_size = rect.size();

                    if (prev_size.x - cur_size.x).abs() > 0.5
                        || (prev_size.y - cur_size.y).abs() > 0.5
                    {
                        if self.slot_a.geo.is_some() || self.slot_b.geo.is_some() {
                            let sx = if prev_size.x > 1.0 {
                                cur_size.x / prev_size.x
                            } else {
                                1.0
                            };
                            let sy = if prev_size.y > 1.0 {
                                cur_size.y / prev_size.y
                            } else {
                                1.0
                            };
                            let factor = (sx * sy).sqrt();
                            if factor.is_finite() && factor > 0.0 {
                                self.zoom = (self.zoom * factor).clamp(ZOOM_MIN, ZOOM_MAX);
                                self.pan *= factor;
                            }
                        }
                        self.last_canvas_size = Some(cur_size);
                    }
                } else {
                    self.last_canvas_size = Some(rect.size());
                }

                // Перетаскивание холста мышью панорамирует вид.
                let response = ui.allocate_rect(rect, egui::Sense::click_and_drag());
                if response.dragged() {
                    self.pan += response.drag_delta();
                }

                // Защита от накопления NaN/Inf в zoom/pan/center_offset
                // (например, из-за экстремальных значений при масштабировании).
                if !self.zoom.is_finite() || self.zoom <= 0.0 {
                    self.zoom = 1.0;
                }
                self.zoom = self.zoom.clamp(ZOOM_MIN, ZOOM_MAX);
                if !self.pan.x.is_finite() || !self.pan.y.is_finite() {
                    self.pan = Vec2::ZERO;
                } else {
                    const PAN_LIMIT: f32 = 1.0e7;
                    self.pan.x = self.pan.x.clamp(-PAN_LIMIT, PAN_LIMIT);
                    self.pan.y = self.pan.y.clamp(-PAN_LIMIT, PAN_LIMIT);
                }
                if !self.center_offset.x.is_finite() || !self.center_offset.y.is_finite() {
                    self.center_offset = Pos2::ZERO;
                }

                // Двойной клик по холсту сбрасывает вид ("вписать в окно").
                if response.double_clicked() {
                    self.fit_view(rect.size());
                    self.last_canvas_size = Some(rect.size());
                }

                let hover_pos = ui.input(|i| i.pointer.hover_pos());

                let cursor_in_canvas = response.hovered();

                let (wheel_zoom, pinch) = if cursor_in_canvas {
                    ui.input(wheel_and_pinch)
                } else {
                    (1.0, 1.0)
                };

                // Колесо мыши и пинч-жест объединяются в общий коэффициент масштабирования.
                let mut zoom_factor: f32 = wheel_zoom;
                if (pinch - 1.0).abs() > 0.001 {
                    zoom_factor *= pinch;
                }

                // Масштабирование выполняется вокруг позиции курсора, а не центра холста:
                // точка под курсором остаётся на месте до и после изменения масштаба.
                if (zoom_factor - 1.0).abs() > 0.0001 {
                    if let Some(pos) = hover_pos.filter(|p| rect.contains(*p)) {
                        let old_center = rect.center() + self.pan;
                        let old_scale = self.zoom;
                        let new_scale = (old_scale * zoom_factor).clamp(ZOOM_MIN, ZOOM_MAX);

                        let new_center_x = pos.x - (pos.x - old_center.x) / old_scale * new_scale;
                        let new_center_y = pos.y - (pos.y - old_center.y) / old_scale * new_scale;

                        self.zoom = new_scale;
                        self.pan = Pos2::new(new_center_x, new_center_y) - rect.center();
                    }
                }

                let painter = ui.painter_at(rect);

                painter.rect_filled(
                    rect,
                    egui::CornerRadius {
                        nw: 0,
                        ne: 0,
                        sw: 0,
                        se: 14,
                    },
                    Color32::from_rgb(252, 252, 253),
                );

                let has_any = self.slot_a.geo.is_some() || self.slot_b.geo.is_some();

                if !has_any {
                    painter.text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        t(self.lang, "empty_state"),
                        egui::FontId::proportional(16.0),
                        Color32::from_rgb(150, 150, 156),
                    );
                } else {
                    let screen_center = rect.center() + self.pan;
                    let scale = self.zoom;
                    let origin = self.center_offset;

                    // Область отсечения (view) чуть больше видимого холста, чтобы объекты,
                    // частично выходящие за край, не исчезали резко. Функции ниже быстро
                    // отбрасывают невидимые примитивы ещё до дорогой отрисовки.
                    let view = rect.expand(64.0);
                    #[inline]
                    fn seg_visible(view: Rect, a: Pos2, b: Pos2) -> bool {
                        if !a.x.is_finite()
                            || !a.y.is_finite()
                            || !b.x.is_finite()
                            || !b.y.is_finite()
                        {
                            return false;
                        }
                        let min_x = a.x.min(b.x);
                        let max_x = a.x.max(b.x);
                        let min_y = a.y.min(b.y);
                        let max_y = a.y.max(b.y);
                        max_x >= view.min.x
                            && min_x <= view.max.x
                            && max_y >= view.min.y
                            && min_y <= view.max.y
                    }
                    #[inline]
                    fn circle_visible(view: Rect, c: Pos2, r: f32) -> bool {
                        if !c.x.is_finite() || !c.y.is_finite() || !r.is_finite() || r <= 0.0 {
                            return false;
                        }
                        c.x + r >= view.min.x
                            && c.x - r <= view.max.x
                            && c.y + r >= view.min.y
                            && c.y - r <= view.max.y
                    }
                    #[inline]
                    fn point_visible(view: Rect, p: Pos2) -> bool {
                        if !p.x.is_finite() || !p.y.is_finite() {
                            return false;
                        }
                        p.x >= view.min.x
                            && p.x <= view.max.x
                            && p.y >= view.min.y
                            && p.y <= view.max.y
                    }
                    #[inline]
                    fn text_visible(
                        view: Rect,
                        anchor: Pos2,
                        px_size: f32,
                        char_count: usize,
                    ) -> bool {
                        if !anchor.x.is_finite()
                            || !anchor.y.is_finite()
                            || !px_size.is_finite()
                            || px_size <= 0.0
                        {
                            return false;
                        }
                        let approx_w = px_size * 0.7 * (char_count.max(1) as f32);
                        let approx_h = px_size * 1.6;
                        anchor.x <= view.max.x
                            && anchor.x + approx_w >= view.min.x
                            && anchor.y + approx_h >= view.min.y
                            && anchor.y - approx_h <= view.max.y
                    }

                    // Перевод мировых координат в экранные (без индивидуального масштаба слота) —
                    // используется, например, для отрисовки различий, уже посчитанных в общем масштабе.
                    let to_screen_prescaled = |p: Pos2| -> Pos2 {
                        Pos2::new(
                            screen_center.x + (p.x - origin.x) * scale,
                            screen_center.y - (p.y - origin.y) * scale,
                        )
                    };

                    // Отрисовка каждого файла: сначала масштабирование геометрии относительно
                    // её собственного центра (slot_scale — независимый регулятор пользователя),
                    // затем перевод в экранные координаты с учётом общего zoom/pan.
                    for slot in [&self.slot_a, &self.slot_b] {
                        if !slot.visible {
                            continue;
                        }
                        if let Some(g) = &slot.geo {
                            let slot_scale = slot.scale;

                            let slot_center = g.bounds.center();
                            let scale_pt = |p: Pos2| -> Pos2 {
                                Pos2::new(
                                    slot_center.x + (p.x - slot_center.x) * slot_scale,
                                    slot_center.y + (p.y - slot_center.y) * slot_scale,
                                )
                            };
                            let to_screen = |p: Pos2| -> Pos2 {
                                let sp = scale_pt(p);
                                Pos2::new(
                                    screen_center.x + (sp.x - origin.x) * scale,
                                    screen_center.y - (sp.y - origin.y) * scale,
                                )
                            };

                            // Отрисовка примитивов слота: цвет уже учитывает прозрачность.
                            let col = style::linear_multiply(slot.color, slot.opacity);
                            let stroke = Stroke::new(1.6, col);
                            for (a, b) in &g.segments {
                                let sa = to_screen(*a);
                                let sb = to_screen(*b);
                                if !sa.x.is_finite()
                                    || !sa.y.is_finite()
                                    || !sb.x.is_finite()
                                    || !sb.y.is_finite()
                                {
                                    continue;
                                }
                                if !seg_visible(view, sa, sb) {
                                    continue;
                                }
                                painter.line_segment([sa, sb], stroke);
                            }
                            for (c, r) in &g.circles {
                                let sc = to_screen(*c);
                                let sr = r * slot_scale * scale;
                                if !sc.x.is_finite()
                                    || !sc.y.is_finite()
                                    || !sr.is_finite()
                                    || sr <= 0.0
                                {
                                    continue;
                                }
                                if !circle_visible(view, sc, sr) {
                                    continue;
                                }
                                circle_stroke_centered(&painter, sc, sr, stroke);
                            }

                            for p in &g.points {
                                let sp = to_screen(*p);
                                if !sp.x.is_finite() || !sp.y.is_finite() {
                                    continue;
                                }
                                if !point_visible(view, sp) {
                                    continue;
                                }
                                painter.circle_filled(sp, 2.2, col);
                            }

                            for (p, h, s, style) in &g.texts {
                                if s.is_empty() {
                                    continue;
                                }

                                let px_size = (h * slot_scale * scale).max(0.05).min(512.0);
                                let sp = to_screen(*p);
                                if !sp.x.is_finite() || !sp.y.is_finite() || !px_size.is_finite() {
                                    continue;
                                }
                                if !text_visible(view, sp, px_size, s.chars().count()) {
                                    continue;
                                }

                                let text_rect = draw_scaled_text(
                                    ui,
                                    &painter,
                                    sp,
                                    egui::Align2::LEFT_BOTTOM,
                                    s,
                                    px_size,
                                    col,
                                );

                                draw_text_decorations(&painter, text_rect, style, col);
                            }
                        }
                    }

                    // Подсветка различий рисуется поверх обоих чертежей: сначала мягкое
                    // "свечение" (glow) для всех различий, затем чёткий контур (core) поверх —
                    // так различия лучше видны даже на пересечениях линий.
                    // Во время перетаскивания ползунка масштаба подсветка временно скрывается
                    // (кэш ещё не пересчитан для нового значения).
                    let dragging_now = self.scale_slider_dragging;
                    if self.diff_enabled && !dragging_now {
                        if let Some(d) = &self.diff_cache {
                            let glow_a =
                                Stroke::new(6.0, style::linear_multiply(self.diff_color_a, 0.30));
                            let core_a = Stroke::new(2.2, self.diff_color_a);
                            let glow_b =
                                Stroke::new(6.0, style::linear_multiply(self.diff_color_b, 0.30));
                            let core_b = Stroke::new(2.2, self.diff_color_b);

                            if self.diff_a_visible {
                                for (a, b) in &d.only_a {
                                    let sa = to_screen_prescaled(*a);
                                    let sb = to_screen_prescaled(*b);
                                    if seg_visible(view, sa, sb) {
                                        painter.line_segment([sa, sb], glow_a);
                                    }
                                }
                            }
                            if self.diff_b_visible {
                                for (a, b) in &d.only_b {
                                    let sa = to_screen_prescaled(*a);
                                    let sb = to_screen_prescaled(*b);
                                    if seg_visible(view, sa, sb) {
                                        painter.line_segment([sa, sb], glow_b);
                                    }
                                }
                            }
                            if self.diff_a_visible {
                                for (c, r) in &d.only_a_circles {
                                    let sc = to_screen_prescaled(*c);
                                    let sr = r * scale;
                                    if circle_visible(view, sc, sr) {
                                        circle_stroke_centered(&painter, sc, sr, glow_a);
                                    }
                                }
                            }
                            if self.diff_b_visible {
                                for (c, r) in &d.only_b_circles {
                                    let sc = to_screen_prescaled(*c);
                                    let sr = r * scale;
                                    if circle_visible(view, sc, sr) {
                                        circle_stroke_centered(&painter, sc, sr, glow_b);
                                    }
                                }
                            }

                            if self.diff_a_visible {
                                for (a, b) in &d.only_a {
                                    let sa = to_screen_prescaled(*a);
                                    let sb = to_screen_prescaled(*b);
                                    if seg_visible(view, sa, sb) {
                                        painter.line_segment([sa, sb], core_a);
                                    }
                                }
                            }
                            if self.diff_b_visible {
                                for (a, b) in &d.only_b {
                                    let sa = to_screen_prescaled(*a);
                                    let sb = to_screen_prescaled(*b);
                                    if seg_visible(view, sa, sb) {
                                        painter.line_segment([sa, sb], core_b);
                                    }
                                }
                            }
                            if self.diff_a_visible {
                                for (c, r) in &d.only_a_circles {
                                    let sc = to_screen_prescaled(*c);
                                    let sr = r * scale;
                                    if circle_visible(view, sc, sr) {
                                        circle_stroke_centered(&painter, sc, sr, core_a);
                                    }
                                }
                            }
                            if self.diff_b_visible {
                                for (c, r) in &d.only_b_circles {
                                    let sc = to_screen_prescaled(*c);
                                    let sr = r * scale;
                                    if circle_visible(view, sc, sr) {
                                        circle_stroke_centered(&painter, sc, sr, core_b);
                                    }
                                }
                            }

                            if self.diff_a_visible {
                                for p in &d.only_a_points {
                                    let sp = to_screen_prescaled(*p);
                                    if !point_visible(view, sp) {
                                        continue;
                                    }
                                    painter.circle_filled(
                                        sp,
                                        6.5,
                                        style::linear_multiply(self.diff_color_a, 0.30),
                                    );
                                    painter.circle_filled(sp, 3.0, self.diff_color_a);
                                }
                            }
                            if self.diff_b_visible {
                                for p in &d.only_b_points {
                                    let sp = to_screen_prescaled(*p);
                                    if !point_visible(view, sp) {
                                        continue;
                                    }
                                    painter.circle_filled(
                                        sp,
                                        6.5,
                                        style::linear_multiply(self.diff_color_b, 0.30),
                                    );
                                    painter.circle_filled(sp, 3.0, self.diff_color_b);
                                }
                            }

                            // Различающиеся надписи отрисовываются с полупрозрачной плашкой-подложкой
                            // под текстом, чтобы они не терялись на фоне обычной геометрии.
                            let a_texts_iter = d
                                .only_a_texts
                                .iter()
                                .filter(|_| self.diff_a_visible)
                                .map(|(p, h, s, style)| (p, h, s, style, self.diff_color_a));
                            let b_texts_iter = d
                                .only_b_texts
                                .iter()
                                .filter(|_| self.diff_b_visible)
                                .map(|(p, h, s, style)| (p, h, s, style, self.diff_color_b));
                            for (p, h, s, style, col) in a_texts_iter.chain(b_texts_iter) {
                                if s.is_empty() {
                                    continue;
                                }
                                let sp = to_screen_prescaled(*p);

                                let px_size = (*h * scale).max(0.05).min(512.0);
                                if !text_visible(view, sp, px_size, s.chars().count()) {
                                    continue;
                                }

                                let size = px_size.round().max(1.0);
                                let font_id = egui::FontId::proportional(size);
                                let galley = ui.fonts_mut(|f| {
                                    f.layout_no_wrap(s.clone(), font_id.clone(), col)
                                });
                                let text_rect = egui::Align2::LEFT_BOTTOM
                                    .anchor_rect(Rect::from_min_size(sp, galley.size()));
                                painter.rect_filled(
                                    text_rect.expand(2.0),
                                    egui::CornerRadius::same(3),
                                    style::linear_multiply(col, 0.25),
                                );
                                painter.text(sp, egui::Align2::LEFT_BOTTOM, s, font_id, col);
                                draw_text_decorations(&painter, text_rect, style, col);
                            }
                        }
                    }
                }
            });
    }
}

impl App {
    // Собственная (без системной рамки) панель заголовка окна: кнопки
    // светофора macOS-стиля, название приложения, переключатель языка и кнопка лицензии.
    fn custom_title_bar(&mut self, ui: &mut egui::Ui) {
        let ctx = &ui.ctx().clone();
        let bar_height = 40.0;

        let frame = egui::Frame::new()
            .fill(Color32::from_rgb(250, 250, 251))
            .corner_radius(egui::CornerRadius {
                nw: 14,
                ne: 14,
                sw: 0,
                se: 0,
            })
            .stroke(Stroke::new(1.0, Color32::from_rgb(228, 228, 232)))
            .inner_margin(egui::Margin {
                left: 13,
                right: 3,
                top: 0,
                bottom: 0,
            });

        let title_bar_interactive =
            !self.show_license && self.error.is_none() && self.file_picker.is_none();

        let title_bar = egui::Panel::top("title_bar")
            .resizable(false)
            .show_separator_line(false)
            .exact_size(bar_height)
            .frame(frame)
            .show(ui, |ui| {
                ui.add_enabled_ui(title_bar_interactive, |ui| {
                    let bar_rect = ui.max_rect();

                    // Перетаскивание за пустую область заголовка перемещает окно,
                    // двойной клик — разворачивает/восстанавливает окно.
                    let bar_drag_resp = ui.interact(
                        bar_rect,
                        ui.id().with("titlebar_drag_full"),
                        egui::Sense::click_and_drag(),
                    );
                    if bar_drag_resp.drag_started() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                    }
                    if bar_drag_resp.double_clicked() {
                        let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
                    }

                    let license_diameter = 20.0;
                    let license_gap = 8.0;
                    let right_margin = 6.0;
                    let combo_width = 96.0;
                    let combo_height = 24.0;
                    let right_group_left = bar_rect.right()
                        - right_margin
                        - license_diameter
                        - license_gap
                        - combo_width;

                    let left_end = ui
                        .with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.spacing_mut().item_spacing.x = 8.0;

                            let close_clicked =
                                traffic_light_button(ui, Color32::from_rgb(255, 95, 87), 0);
                            let min_clicked =
                                traffic_light_button(ui, Color32::from_rgb(255, 189, 46), 1);
                            let max_clicked =
                                traffic_light_button(ui, Color32::from_rgb(40, 201, 64), 2);

                            if close_clicked {
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            }
                            if min_clicked {
                                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                            }
                            if max_clicked {
                                let maximized =
                                    ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                                ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
                            }

                            ui.add_space(14.0);
                            ui.spacing_mut().item_spacing.x = 6.0;
                            ui.label(egui::RichText::new(platform::APP_NAME).size(15.0).strong());

                            // Ширина подзаголовка кэшируется по языку, чтобы не пересчитывать
                            // раскладку текста (layout_no_wrap) на каждом кадре.
                            let subtitle_text = t(self.lang, "subtitle");
                            let subtitle_width = match self.subtitle_width_cache {
                                Some((lang, w)) if lang == self.lang => w,
                                _ => {
                                    let galley = ui.fonts_mut(|f| {
                                        f.layout_no_wrap(
                                            subtitle_text.to_string(),
                                            egui::FontId::proportional(12.0),
                                            Color32::PLACEHOLDER,
                                        )
                                    });
                                    let w = galley.size().x;
                                    self.subtitle_width_cache = Some((self.lang, w));
                                    w
                                }
                            };
                            let would_end = ui.min_rect().right() + 6.0 + subtitle_width;
                            if would_end < right_group_left - 12.0 {
                                ui.label(egui::RichText::new(subtitle_text).size(12.0).weak());
                            }
                        })
                        .response
                        .rect
                        .right();

                    let license_rect = Rect::from_min_size(
                        Pos2::new(
                            bar_rect.right() - right_margin - license_diameter,
                            bar_rect.center().y - license_diameter / 2.0,
                        ),
                        Vec2::splat(license_diameter),
                    );

                    let combo_rect = Rect::from_min_size(
                        Pos2::new(
                            license_rect.left() - license_gap - combo_width,
                            bar_rect.center().y - combo_height / 2.0,
                        ),
                        Vec2::new(combo_width, combo_height),
                    );
                    ui.scope_builder(egui::UiBuilder::new().max_rect(combo_rect), |ui| {
                        ui.style_mut().spacing.button_padding = egui::vec2(8.0, 4.0);
                        egui::ComboBox::from_id_salt("lang_selector")
                            .width(combo_width)
                            .selected_text(self.lang.label())
                            .show_ui(ui, |ui| {
                                for l in Lang::ALL {
                                    ui.selectable_value(&mut self.lang, l, l.label());
                                }
                            });
                    });

                    let license_resp = ui
                        .scope_builder(egui::UiBuilder::new().max_rect(license_rect), |ui| {
                            license_button(ui, license_diameter)
                        })
                        .inner;
                    if license_resp.clicked() {
                        self.show_license = true;
                    }
                    license_resp.on_hover_text(t(self.lang, "license_button_tooltip"));

                    let _ = left_end;
                });
            });
        paint_panel_separator(ui, title_bar.response.rect, PanelEdge::Bottom);
    }

    // Модальный диалог выбора кодировки текста, показывается, когда
    // автоопределение не смогло уверенно декодировать файл.
    fn encoding_choice_window(&mut self, ctx: &egui::Context) {
        let Some(pending) = &self.pending_encoding else {
            return;
        };
        let lang = self.lang;
        let screen = ctx.content_rect();

        const MARGIN: f32 = 24.0;
        let window_size = Vec2::new(
            380.0_f32.min((screen.width() - MARGIN * 2.0).max(240.0)),
            (100.0 + geometry::ENCODING_CHOICES.len() as f32 * 26.0 + 90.0)
                .min((screen.height() - MARGIN * 2.0).max(260.0)),
        );
        let win_rect = Rect::from_center_size(screen.center(), window_size);

        let mut chosen: Option<usize> = None;
        let mut confirm_clicked = false;
        let mut cancel_clicked = false;
        let file_name = pending
            .path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| pending.path.display().to_string());
        let selected = pending.selected;

        egui::Area::new(egui::Id::new("encoding_modal_layer"))
            .order(egui::Order::Foreground)
            .fixed_pos(Pos2::ZERO)
            .interactable(true)
            .show(ctx, |ui| {
                ui.allocate_response(screen.size(), egui::Sense::click_and_drag());
                ui.painter()
                    .rect_filled(screen, 0.0, Color32::from_black_alpha(90));

                ui.scope_builder(egui::UiBuilder::new().max_rect(win_rect), |ui| {
                    ui.set_width(window_size.x);
                    ui.set_height(window_size.y);

                    egui::Frame::new()
                        .fill(Color32::from_rgb(246, 246, 248))
                        .corner_radius(egui::CornerRadius::same(14))
                        .stroke(Stroke::new(1.0, Color32::from_rgb(228, 228, 232)))
                        .inner_margin(egui::Margin::same(17))
                        .show(ui, |ui| {
                            ui.set_width(window_size.x - 36.0);

                            ui.label(
                                egui::RichText::new(t(lang, "encoding_dialog_title")).strong(),
                            );
                            ui.add_space(6.0);
                            ui.label(egui::RichText::new(file_name.as_str()).small().weak());
                            ui.add_space(4.0);
                            ui.label(egui::RichText::new(t(lang, "encoding_dialog_desc")).small());
                            ui.add_space(10.0);

                            egui::ScrollArea::vertical()
                                .max_height(window_size.y - 150.0)
                                .auto_shrink([false, false])
                                .scroll_bar_visibility(
                                    egui::scroll_area::ScrollBarVisibility::AlwaysHidden,
                                )
                                .show(ui, |ui| {
                                    for i in 0..geometry::ENCODING_CHOICES.len() {
                                        let label = encoding_choice_label(lang, i);
                                        if ui.radio(selected == i, label).clicked() {
                                            chosen = Some(i);
                                        }
                                    }
                                });

                            ui.add_space(12.0);
                            ui.horizontal(|ui| {
                                if ui
                                    .add(
                                        egui::Button::new(
                                            egui::RichText::new(t(lang, "encoding_ok"))
                                                .color(Color32::WHITE),
                                        )
                                        .fill(style::COLOR_A_DEFAULT),
                                    )
                                    .clicked()
                                {
                                    confirm_clicked = true;
                                }
                                if ui.button(t(lang, "encoding_cancel")).clicked() {
                                    cancel_clicked = true;
                                }
                            });
                        });
                });
            });

        if let Some(i) = chosen {
            if let Some(p) = &mut self.pending_encoding {
                p.selected = i;
            }
        }
        if cancel_clicked {
            self.pending_encoding = None;
        }
        if confirm_clicked {
            if let Some(p) = self.pending_encoding.take() {
                let enc_idx = p.selected;
                let enc = geometry::ENCODING_CHOICES[enc_idx].1;

                let slot = if p.slot_index == 0 {
                    &mut self.slot_a
                } else {
                    &mut self.slot_b
                };
                slot.encoding_choice = Some(enc_idx);
                self.load_path_into_slot(p.path, p.slot_index, Some(enc));
            }
        }
    }

    // Открыто ли какое-либо модальное окно (тогда окно нельзя растягивать за края).
    fn modal_open(&self) -> bool {
        self.show_license
            || self.error.is_some()
            || self.pending_encoding.is_some()
            || self.file_picker.is_some()
    }

    // Окно встроенного обзора файлов (запасной вариант системного диалога).
    fn file_picker_window(&mut self, ctx: &egui::Context) {
        let Some(picker) = &mut self.file_picker else {
            return;
        };
        match file_picker::show(picker, ctx, self.lang) {
            file_picker::PickerAction::None => {}
            file_picker::PickerAction::Cancel => self.file_picker = None,
            file_picker::PickerAction::Open(path) => {
                if let Some(picker) = self.file_picker.take() {
                    self.load_path_into_slot(path, picker.slot_index, picker.forced_encoding);
                }
            }
        }
    }

    // Модальное окно со сведениями о лицензиях используемых библиотек и шрифтов.
    fn license_window(&mut self, ctx: &egui::Context) {
        if !self.show_license {
            return;
        }

        let lang = self.lang;
        let render_info = &self.render_info;
        let closed = modal_window(
            ctx,
            "license",
            t(lang, "license_title"),
            Vec2::new(460.0, 480.0),
            |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new(t(lang, "license_libraries")).strong());
                        ui.add_space(4.0);
                        for (name, version, license) in LIBRARY_LICENSES {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(*name).strong());
                                ui.weak(format!("v{}", version));
                                ui.label(format!("— {}", license));
                            });
                        }

                        ui.add_space(14.0);
                        ui.separator();
                        ui.add_space(10.0);

                        ui.label(egui::RichText::new(t(lang, "license_fonts")).strong());
                        ui.add_space(4.0);
                        ui.label(t(lang, "license_fonts_body"));
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            ui.label("•");
                            ui.label("Google Sans — SIL Open Font License 1.1 (Google LLC)");
                        });
                        ui.horizontal(|ui| {
                            ui.label("•");
                            ui.label("Noto Sans JP — SIL Open Font License 1.1 (Google LLC)");
                        });
                        ui.add_space(6.0);
                        ui.label(t(lang, "license_fonts_egui"));
                        for font in EGUI_FONT_LICENSES {
                            ui.horizontal(|ui| {
                                ui.label("•");
                                ui.label(*font);
                            });
                        }
                        ui.add_space(4.0);
                        ui.hyperlink_to("openfontlicense.org", "https://openfontlicense.org");

                        ui.add_space(14.0);
                        ui.separator();
                        ui.add_space(10.0);

                        ui.label(egui::RichText::new(t(lang, "license_software")).strong());
                        ui.add_space(4.0);
                        ui.label(t(lang, "license_software_body"));
                        ui.add_space(6.0);
                        ui.hyperlink_to(
                            "github.com/Rafych/pyatorka",
                            "https://github.com/Rafych/pyatorka",
                        );
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.label("•");
                            ui.label("License: MIT License");
                        });
                        ui.horizontal(|ui| {
                            ui.label("•");
                            ui.label("Author: Rafych");
                        });
                        ui.add_space(8.0);
                        ui.label(t(lang, "license_full_texts"));
                        ui.hyperlink_to("THIRD_PARTY_LICENSES.html", THIRD_PARTY_LICENSES_URL);

                        // Способ отрисовки: GPU или CPU и конкретное устройство.
                        if let Some(info) = render_info {
                            ui.add_space(14.0);
                            ui.separator();
                            ui.add_space(10.0);
                            ui.label(egui::RichText::new(t(lang, "render_title")).strong());
                            ui.add_space(4.0);
                            let kind = if info.on_cpu {
                                t(lang, "render_cpu")
                            } else {
                                t(lang, "render_gpu")
                            };
                            ui.label(kind);
                            ui.label(
                                egui::RichText::new(format!("{} ({})", info.device, info.api))
                                    .small()
                                    .weak(),
                            );
                        }

                        ui.add_space(14.0);
                    });
            },
        );

        if closed {
            self.show_license = false;
        }
    }

    // Модальное окно с сообщением об ошибке загрузки файла.
    fn error_window(&mut self, ctx: &egui::Context) {
        let Some(error) = &self.error else {
            return;
        };

        let lang = self.lang;
        let mut ok_clicked = false;
        let closed = modal_window(
            ctx,
            "load_error",
            t(lang, "load_error"),
            Vec2::new(420.0, 240.0),
            |ui| {
                const BUTTON_HEIGHT: f32 = 32.0;
                let text_height =
                    (ui.available_height() - BUTTON_HEIGHT - ui.spacing().item_spacing.y).max(0.0);
                egui::ScrollArea::vertical()
                    .max_height(text_height)
                    .auto_shrink([false, false])
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new(error.file_name.as_str()).small().weak());
                        ui.add_space(4.0);
                        ui.colored_label(Color32::from_rgb(209, 40, 30), error.message.as_str());
                    });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add_sized(
                            [96.0, BUTTON_HEIGHT],
                            egui::Button::new(
                                egui::RichText::new(t(lang, "error_ok")).color(Color32::WHITE),
                            )
                            .fill(style::ACCENT)
                            .corner_radius(egui::CornerRadius::same(8)),
                        )
                        .clicked()
                    {
                        ok_clicked = true;
                    }
                });
            },
        );

        if closed || ok_clicked || ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
            self.error = None;
        }
    }

    // Содержимое левой панели: блоки файлов A/B, панель различий и общие кнопки.
    fn control_panel_body(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical()
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.add_space(4.0);

                let open_button_width = ui.available_width();
                self.slot_panel(ui, 0, open_button_width);
                ui.add_space(14.0);
                ui.separator();
                ui.add_space(14.0);
                self.slot_panel(ui, 1, open_button_width);
                ui.add_space(14.0);
                ui.separator();
                ui.add_space(14.0);
                self.diff_panel(ui);
                ui.add_space(18.0);
                ui.separator();
                ui.add_space(10.0);

                if ui
                    .add_sized(
                        [ui.available_width(), 34.0],
                        egui::Button::new(t(self.lang, "fit")),
                    )
                    .clicked()
                {
                    self.needs_fit = true;
                }

                ui.add_space(6.0);
                if ui
                    .add_sized(
                        [ui.available_width(), 34.0],
                        egui::Button::new(t(self.lang, "reset_settings")),
                    )
                    .clicked()
                {
                    self.slot_a.color = style::COLOR_A_DEFAULT;
                    self.slot_a.opacity = 1.0;
                    self.slot_b.color = style::COLOR_B_DEFAULT;
                    self.slot_b.opacity = 1.0;
                    self.diff_color_a = style::DIFF_COLOR_A_DEFAULT;
                    self.diff_color_b = style::DIFF_COLOR_B_DEFAULT;
                }

                ui.add_space(16.0);
                ui.label(egui::RichText::new(t(self.lang, "tip")).small().weak());
            });
    }

    // Панель одного файла (A или B): кнопка открытия, имя файла, число объектов,
    // регуляторы видимости/прозрачности/цвета/масштаба.
    fn slot_panel(&mut self, ui: &mut egui::Ui, idx: usize, open_button_width: f32) {
        let label = if idx == 0 {
            t(self.lang, "file_a")
        } else {
            t(self.lang, "file_b")
        };

        egui::Frame::new()
            .fill(Color32::from_rgb(255, 255, 255))
            .corner_radius(egui::CornerRadius::same(10))
            .stroke(Stroke::new(1.0, Color32::from_rgb(230, 230, 235)))
            .inner_margin(egui::Margin::same(11))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    let dot_color = if idx == 0 {
                        self.slot_a.color
                    } else {
                        self.slot_b.color
                    };
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(10.0), egui::Sense::hover());
                    ui.painter().circle_filled(rect.center(), 5.0, dot_color);
                    ui.label(egui::RichText::new(label).strong());
                });

                ui.add_space(6.0);

                let button_width = (open_button_width - 24.0).max(0.0);

                let narrow_width = button_width;
                let open_clicked = ui
                    .add_sized(
                        [narrow_width, 32.0],
                        egui::Button::new(
                            egui::RichText::new(t(self.lang, "open_file")).color(Color32::WHITE),
                        )
                        .fill(style::ACCENT)
                        .corner_radius(egui::CornerRadius::same(8)),
                    )
                    .clicked();

                let slot = if idx == 0 {
                    &mut self.slot_a
                } else {
                    &mut self.slot_b
                };

                ui.add_space(4.0);
                match &slot.path {
                    Some(p) => {
                        let name = std::path::Path::new(p)
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| p.clone());
                        ui.label(egui::RichText::new(name).small());
                        if let Some(g) = &slot.geo {
                            ui.label(
                                egui::RichText::new(format!(
                                    "{}: {}",
                                    t(self.lang, "entities"),
                                    g.entity_count
                                ))
                                .small()
                                .weak(),
                            );
                            ui.label(egui::RichText::new(&g.encoding_label).small().weak());
                        }
                    }
                    None => {
                        ui.label(egui::RichText::new(t(self.lang, "no_file")).small().weak());
                    }
                }

                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.checkbox(&mut slot.visible, t(self.lang, "visible"));
                    ui.add_space(8.0);
                    ui.label(t(self.lang, "color"));

                    egui::widgets::color_picker::color_edit_button_srgba(
                        ui,
                        &mut slot.color,
                        egui::widgets::color_picker::Alpha::Opaque,
                    );
                });

                ui.add_space(4.0);
                ui.add(
                    egui::Slider::new(&mut slot.opacity, 0.0..=1.0).text(t(self.lang, "opacity")),
                );

                ui.add_space(4.0);
                // Логарифмический ползунок масштаба удобнее для диапазона 0.01x–10x.
                // Флаг scale_slider_dragging используется, чтобы на время перетаскивания
                // скрыть подсветку различий (см. update_diff_cache/update).
                let scale_resp = ui.add(
                    egui::Slider::new(&mut slot.scale, 0.01..=10.0)
                        .logarithmic(true)
                        .text(t(self.lang, "scale_ratio")),
                );
                if scale_resp.dragged() || scale_resp.is_pointer_button_down_on() {
                    self.scale_slider_dragging = true;
                }
                if ui.small_button(t(self.lang, "scale_reset")).clicked() {
                    slot.scale = 1.0;
                }

                ui.add_space(8.0);
                ui.label(t(self.lang, "encoding_label"));
                ui.add_space(2.0);
                let mut encoding_changed = false;
                let current_label = match slot.encoding_choice {
                    None => t(self.lang, "encoding_auto").to_string(),
                    Some(i) => encoding_choice_label(self.lang, i),
                };

                egui::ComboBox::from_id_salt(("encoding_combo", idx))
                    .selected_text(current_label)
                    .width(narrow_width)
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_label(
                                slot.encoding_choice.is_none(),
                                t(self.lang, "encoding_auto"),
                            )
                            .clicked()
                        {
                            slot.encoding_choice = None;
                            encoding_changed = true;
                        }
                        for i in 0..geometry::ENCODING_CHOICES.len() {
                            let label = encoding_choice_label(self.lang, i);
                            if ui
                                .selectable_label(slot.encoding_choice == Some(i), label)
                                .clicked()
                            {
                                slot.encoding_choice = Some(i);
                                encoding_changed = true;
                            }
                        }
                    });

                let current_encoding = slot
                    .encoding_choice
                    .map(|i| geometry::ENCODING_CHOICES[i].1);
                let existing_path = slot.path.clone().map(std::path::PathBuf::from);

                // Смена кодировки вручную перезагружает уже выбранный файл с новой кодировкой.
                if open_clicked {
                    self.open_dialog(idx, current_encoding);
                } else if encoding_changed {
                    if let Some(path) = existing_path {
                        self.load_path_into_slot(path, idx, current_encoding);
                    }
                }
            });
    }

    // Панель настроек подсветки различий: включение, видимость и цвет для A/B.
    fn diff_panel(&mut self, ui: &mut egui::Ui) {
        egui::Frame::new()
            .fill(Color32::from_rgb(255, 255, 255))
            .corner_radius(egui::CornerRadius::same(10))
            .stroke(Stroke::new(1.0, Color32::from_rgb(230, 230, 235)))
            .inner_margin(egui::Margin::same(11))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());

                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(10.0), egui::Sense::hover());
                    ui.painter()
                        .circle_filled(rect.center(), 5.0, self.diff_color_a);
                    ui.label(egui::RichText::new(t(self.lang, "diff_title")).strong());
                });

                ui.add_space(6.0);
                ui.checkbox(&mut self.diff_enabled, t(self.lang, "diff_enable"));

                ui.add_space(4.0);
                ui.add_enabled_ui(self.diff_enabled, |ui| {
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.diff_a_visible, "");
                        if ui
                            .add(
                                egui::Label::new(t(self.lang, "diff_color_a"))
                                    .sense(egui::Sense::click()),
                            )
                            .clicked()
                        {
                            self.diff_a_visible = !self.diff_a_visible;
                        }
                        egui::widgets::color_picker::color_edit_button_srgba(
                            ui,
                            &mut self.diff_color_a,
                            egui::widgets::color_picker::Alpha::Opaque,
                        );
                        if ui.small_button(t(self.lang, "diff_color_reset")).clicked() {
                            self.diff_color_a = style::DIFF_COLOR_A_DEFAULT;
                        }
                    });
                    ui.add_space(2.0);
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.diff_b_visible, "");
                        if ui
                            .add(
                                egui::Label::new(t(self.lang, "diff_color_b"))
                                    .sense(egui::Sense::click()),
                            )
                            .clicked()
                        {
                            self.diff_b_visible = !self.diff_b_visible;
                        }
                        egui::widgets::color_picker::color_edit_button_srgba(
                            ui,
                            &mut self.diff_color_b,
                            egui::widgets::color_picker::Alpha::Opaque,
                        );
                        if ui.small_button(t(self.lang, "diff_color_reset")).clicked() {
                            self.diff_color_b = style::DIFF_COLOR_B_DEFAULT;
                        }
                    });
                });

                let both_loaded = self.slot_a.geo.is_some() && self.slot_b.geo.is_some();
                if self.diff_enabled && !both_loaded {
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(t(self.lang, "diff_need_both"))
                            .small()
                            .weak(),
                    );
                } else {
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new(t(self.lang, "diff_tip")).small().weak());
                }
            });
    }
}

// Край панели, вдоль которого рисуется разделительная линия.
enum PanelEdge {
    Bottom,
    Right,
}

// Рисует тонкую разделительную линию вдоль края панели — изнутри панели, поверх
// обводки её рамки (так выглядели панели в прежнем оформлении). Встроенная линия
// egui рисуется снаружи панели, где сквозь зазор виден прозрачный фон окна.
fn paint_panel_separator(ui: &egui::Ui, rect: Rect, edge: PanelEdge) {
    let painter = ui.painter();
    let stroke = ui.visuals().widgets.noninteractive.bg_stroke;
    match edge {
        PanelEdge::Bottom => {
            let y = painter.round_to_pixel_center(rect.bottom() - 1.0);
            painter.hline(rect.x_range(), y, stroke);
        }
        PanelEdge::Right => {
            let x = painter.round_to_pixel_center(rect.right() - 1.0);
            painter.vline(x, rect.y_range(), stroke);
        }
    }
}

// Внешний прямоугольник панели, сохранённый egui в предыдущем кадре.
fn panel_outer_rect(ctx: &egui::Context, id: egui::Id) -> Option<Rect> {
    egui::containers::panel::PanelState::load(ctx, id).map(|s| s.outer_rect)
}

// Панель, подстраивающаяся под содержимое, применяет новый размер только со
// следующего кадра (до этого содержимое обрезается). Если размер вдоль оси `axis`
// (0 — ширина, 1 — высота) изменился, кадр сразу перерисовывается заново,
// чтобы обрезанное состояние не попало на экран.
fn discard_if_panel_resized(ctx: &egui::Context, id: egui::Id, before: Option<Rect>, axis: usize) {
    let after = panel_outer_rect(ctx, id);
    let resized = match (before, after) {
        (Some(b), Some(a)) => (b.size()[axis] - a.size()[axis]).abs() > 0.5,
        (None, Some(_)) => true,
        _ => false,
    };
    if resized {
        ctx.request_discard("размер панели изменён под её содержимое");
    }
}

// Сколько точек (points) прокрутки соответствует одной строке колеса мыши.
const POINTS_PER_SCROLL_LINE: f32 = 50.0;

// Насколько меняется масштаб холста на одну точку (point) прокрутки колесом.
const WHEEL_ZOOM_PER_POINT: f32 = 0.0015;

// Возвращает за текущий кадр коэффициент масштабирования колесом и коэффициент
// масштабирования жестом/Ctrl+колесом.
// Каждое событие колеса учитывается отдельно: если кадры идут редко (например, при
// отрисовке на CPU) и за кадр приходит несколько щелчков колеса, масштаб меняется
// так же, как если бы они пришли в разных кадрах.
// Колесо с Ctrl (Cmd) работает как масштабирование, с Shift — как горизонтальная
// прокрутка (на масштаб холста тогда не влияет).
fn wheel_and_pinch(i: &egui::InputState) -> (f32, f32) {
    let mut wheel_zoom = 1.0_f32;
    let mut zoom = 1.0_f32;
    for event in &i.events {
        match event {
            egui::Event::MouseWheel {
                unit,
                delta,
                modifiers,
                ..
            } => {
                let delta = match unit {
                    egui::MouseWheelUnit::Point => *delta,
                    egui::MouseWheelUnit::Line => *delta * POINTS_PER_SCROLL_LINE,
                    egui::MouseWheelUnit::Page => *delta * i.viewport_rect().height(),
                };
                if modifiers.ctrl || modifiers.command {
                    zoom *= (delta.y / 200.0).exp();
                } else if !modifiers.shift && delta.y != 0.0 {
                    wheel_zoom *= (1.0 + delta.y * WHEEL_ZOOM_PER_POINT).clamp(0.5, 1.5);
                }
            }
            egui::Event::Zoom(factor) => zoom *= *factor,
            _ => {}
        }
    }
    // Пинч на сенсорном экране (несколько касаний) заменяет остальные источники масштаба.
    let pinch = i.multi_touch().map_or(zoom, |touch| touch.zoom_delta);
    (wheel_zoom, pinch)
}

// Модальное окно в общем стиле приложения: затемнение фона, рамка со скруглёнными
// углами, заголовок с кнопкой закрытия и область содержимого `add_body`.
// Возвращает true, если окно закрыли (крестиком или клавишей Escape).
fn modal_window(
    ctx: &egui::Context,
    id: &str,
    title: &str,
    size: Vec2,
    add_body: impl FnOnce(&mut egui::Ui),
) -> bool {
    let screen = ctx.content_rect();

    const MARGIN: f32 = 24.0;
    let window_size = Vec2::new(
        size.x.min((screen.width() - MARGIN * 2.0).max(240.0)),
        size.y.min((screen.height() - MARGIN * 2.0).max(260.0)),
    );

    let win_rect = Rect::from_center_size(screen.center(), window_size);
    let mut close_clicked = false;

    egui::Area::new(egui::Id::new(id).with("modal_layer"))
        .order(egui::Order::Foreground)
        .fixed_pos(Pos2::ZERO)
        .interactable(true)
        .show(ctx, |ui| {
            ui.allocate_response(screen.size(), egui::Sense::click_and_drag());
            ui.painter()
                .rect_filled(screen, 0.0, Color32::from_black_alpha(90));

            ui.scope_builder(egui::UiBuilder::new().max_rect(win_rect), |ui| {
                ui.set_width(window_size.x);
                ui.set_height(window_size.y);

                egui::Frame::new()
                    .fill(Color32::from_rgb(246, 246, 248))
                    .corner_radius(egui::CornerRadius::same(14))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(228, 228, 232)))
                    .show(ui, |ui| {
                        // Обводка (1 px с каждой стороны) входит в размер рамки.
                        ui.set_width(window_size.x - 2.0);
                        ui.set_height(window_size.y - 2.0);

                        let bar_height = 36.0;

                        let bar_frame = egui::Frame::new()
                            .fill(Color32::from_rgb(250, 250, 251))
                            .corner_radius(egui::CornerRadius {
                                nw: 14,
                                ne: 14,
                                sw: 0,
                                se: 0,
                            });

                        let title_bar = egui::Panel::top(egui::Id::new(id).with("modal_title_bar"))
                            .resizable(false)
                            .show_separator_line(false)
                            .exact_size(bar_height)
                            .frame(bar_frame)
                            .show(ui, |ui| {
                                let bar_rect = ui.max_rect();

                                ui.painter().text(
                                    bar_rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    title,
                                    egui::FontId::proportional(14.0),
                                    Color32::from_rgb(40, 40, 44),
                                );

                                let close_diameter = 22.0;
                                let right_margin = 8.0;
                                let close_rect = Rect::from_min_size(
                                    Pos2::new(
                                        bar_rect.right() - right_margin - close_diameter,
                                        bar_rect.center().y - close_diameter / 2.0,
                                    ),
                                    Vec2::splat(close_diameter),
                                );
                                let resp = ui
                                    .scope_builder(
                                        egui::UiBuilder::new().max_rect(close_rect),
                                        |ui| close_x_button(ui, close_diameter),
                                    )
                                    .inner;
                                if resp.clicked() {
                                    close_clicked = true;
                                }
                            });
                        paint_panel_separator(ui, title_bar.response.rect, PanelEdge::Bottom);

                        let body_rounding = egui::CornerRadius {
                            nw: 0,
                            ne: 0,
                            sw: 14,
                            se: 14,
                        };
                        let body_frame = egui::Frame::new()
                            .fill(Color32::from_rgb(246, 246, 248))
                            .corner_radius(body_rounding)
                            .inner_margin(egui::Margin::same(16));

                        egui::CentralPanel::default()
                            .frame(body_frame)
                            .show(ui, add_body);
                    });
            });
        });

    close_clicked || ctx.input(|i| i.key_pressed(egui::Key::Escape))
}

// Реализует изменение размера окна без системной рамки: определяет,
// когда курсор находится у края окна, и запускает системный resize-drag.
fn handle_resize_edges(ctx: &egui::Context) {
    let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
    if maximized {
        return;
    }

    let screen = ctx.content_rect();
    const BORDER: f32 = 8.0;
    const MIN_SIZE: Vec2 = Vec2::new(420.0, 480.0);

    let area = egui::Area::new(egui::Id::new("resize_overlay"))
        .order(egui::Order::Foreground)
        .fixed_pos(Pos2::ZERO)
        .interactable(true);

    area.show(ctx, |ui| {
        let east_rect = Rect::from_min_max(
            Pos2::new(screen.right() - BORDER, screen.top()),
            Pos2::new(screen.right(), screen.bottom() - BORDER),
        );
        let south_rect = Rect::from_min_max(
            Pos2::new(screen.left(), screen.bottom() - BORDER),
            Pos2::new(screen.right() - BORDER, screen.bottom()),
        );
        let corner_rect = Rect::from_min_max(
            Pos2::new(screen.right() - BORDER, screen.bottom() - BORDER),
            Pos2::new(screen.right(), screen.bottom()),
        );

        // Три невидимые области-ловушки: правый край, нижний край и угол.
        let east = ui.interact(east_rect, ui.id().with("resize_east"), egui::Sense::drag());
        let south = ui.interact(
            south_rect,
            ui.id().with("resize_south"),
            egui::Sense::drag(),
        );
        let corner = ui.interact(
            corner_rect,
            ui.id().with("resize_corner"),
            egui::Sense::drag(),
        );

        if east.hovered() || east.dragged() {
            ui.set_cursor_icon(egui::CursorIcon::ResizeEast);
        }
        if south.hovered() || south.dragged() {
            ui.set_cursor_icon(egui::CursorIcon::ResizeSouth);
        }
        if corner.hovered() || corner.dragged() {
            ui.set_cursor_icon(egui::CursorIcon::ResizeSouthEast);
        }

        let mut new_size = screen.size();
        let mut changed = false;

        if corner.dragged() {
            new_size += corner.drag_delta();
            changed = true;
        } else {
            if east.dragged() {
                new_size.x += east.drag_delta().x;
                changed = true;
            }
            if south.dragged() {
                new_size.y += south.drag_delta().y;
                changed = true;
            }
        }

        if changed {
            new_size = new_size.max(MIN_SIZE);
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(new_size));
        }
    });
}

// Круглая иконка "i" — кнопка открытия окна лицензий в заголовке.
fn license_button(ui: &mut egui::Ui, diameter: f32) -> egui::Response {
    let size = Vec2::splat(diameter);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let painter = ui.painter();

    let base = Color32::from_rgb(120, 120, 128);
    let hover = Color32::from_rgb(60, 60, 66);
    let color = if response.hovered() { hover } else { base };

    let center = rect.center();
    let r = diameter / 2.0;
    circle_stroke_centered(painter, center, r - 1.0, Stroke::new(1.4, color));

    let dot_r = 1.3;
    let bar_top = center.y - r * 0.45;
    let bar_bottom = center.y + r * 0.12;
    painter.line_segment(
        [
            Pos2::new(center.x, bar_top),
            Pos2::new(center.x, bar_bottom),
        ],
        Stroke::new(1.6, color),
    );
    painter.circle_filled(Pos2::new(center.x, center.y + r * 0.45), dot_r, color);

    response
}

// Круглая кнопка закрытия (крестик) для модальных окон.
fn close_x_button(ui: &mut egui::Ui, diameter: f32) -> egui::Response {
    let size = Vec2::splat(diameter);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let painter = ui.painter();

    let hover = response.hovered();
    if hover {
        painter.circle_filled(
            rect.center(),
            diameter / 2.0,
            Color32::from_rgb(232, 232, 236),
        );
    }

    let color = if hover {
        Color32::from_rgb(40, 40, 44)
    } else {
        Color32::from_rgb(120, 120, 128)
    };
    let r = diameter * 0.22;
    let c = rect.center();
    let stroke = Stroke::new(1.5, color);
    painter.line_segment([c + Vec2::new(-r, -r), c + Vec2::new(r, r)], stroke);
    painter.line_segment([c + Vec2::new(-r, r), c + Vec2::new(r, -r)], stroke);

    response
}

// Кнопка в стиле "светофора" macOS (закрыть/свернуть/развернуть).
// kind: 0 — крестик (закрыть), 1 — минус (свернуть), иное — квадрат (развернуть).
fn traffic_light_button(ui: &mut egui::Ui, color: Color32, kind: u8) -> bool {
    let size = Vec2::splat(14.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let painter = ui.painter();
    painter.circle_filled(rect.center(), size.x / 2.0, color);

    if response.hovered() {
        let c = rect.center();
        let stroke = Stroke::new(1.4, Color32::from_black_alpha(170));
        match kind {
            0 => {
                let r = 3.2;
                painter.line_segment([c + Vec2::new(-r, -r), c + Vec2::new(r, r)], stroke);
                painter.line_segment([c + Vec2::new(-r, r), c + Vec2::new(r, -r)], stroke);
            }
            1 => {
                let r = 3.4;
                painter.line_segment([c + Vec2::new(-r, 0.0), c + Vec2::new(r, 0.0)], stroke);
            }
            _ => {
                let r = 2.6;
                painter.rect_stroke(
                    Rect::from_center_size(c, Vec2::splat(r * 2.0)),
                    1.0,
                    stroke,
                    egui::StrokeKind::Middle,
                );
            }
        }
    }
    response.clicked()
}

// Список сторонних библиотек и их лицензий для окна "О программе".
// Здесь перечислены прямые зависимости и wgpu; полный список всех входящих
// библиотек с текстами лицензий — в THIRD_PARTY_LICENSES.html (см. licenses/).
const LIBRARY_LICENSES: &[(&str, &str, &str)] = &[
    ("eframe", "0.36", "MIT OR Apache-2.0"),
    ("egui", "0.36", "MIT OR Apache-2.0"),
    ("wgpu", "30", "MIT OR Apache-2.0"),
    ("winit", "0.30", "Apache-2.0"),
    ("dxf", "0.6", "MIT"),
    ("rfd", "0.17", "MIT"),
    ("encoding_rs", "0.8", "(MIT OR Apache-2.0) AND BSD-3-Clause"),
    ("chardetng", "1", "MIT OR Apache-2.0"),
    ("serde", "1", "MIT OR Apache-2.0"),
    ("image", "0.25", "MIT OR Apache-2.0"),
    ("log", "0.4", "MIT OR Apache-2.0"),
    ("libc", "0.2", "MIT OR Apache-2.0"),
];

// Шрифты, которые библиотека egui встраивает в приложение (крейт epaint_default_fonts).
const EGUI_FONT_LICENSES: &[&str] = &[
    "Hack — MIT License / Bitstream Vera License",
    "Ubuntu Light — Ubuntu Font Licence 1.0",
    "Noto Emoji — SIL Open Font License 1.1",
    "emoji-icon-font — MIT License",
];

// Страница с полными текстами лицензий всех входящих библиотек и шрифтов.
const THIRD_PARTY_LICENSES_URL: &str =
    "https://rafych.github.io/pyatorka/THIRD_PARTY_LICENSES.html";

// HAS_ICON и содержимое иконки подготавливаются build.rs (см. resolve_icon).
include!(concat!(env!("OUT_DIR"), "/icon_info.rs"));
const ICON_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/icon.png"));

// Декодирует встроенную PNG-иконку окна, если она была предоставлена при сборке.
fn load_app_icon() -> Option<egui::IconData> {
    if !HAS_ICON {
        return None;
    }
    let img = image::load_from_memory(ICON_BYTES).ok()?.into_rgba8();
    let (width, height) = img.dimensions();
    Some(egui::IconData {
        rgba: img.into_raw(),
        width,
        height,
    })
}

// Точка входа: настраивает окно (без системной рамки, с прозрачностью для
// собственных скруглённых углов) и запускает цикл eframe.
fn main() -> eframe::Result<()> {
    file_picker::install_dialog_logger();
    startup::install_panic_hook();

    // Без нужных системных компонентов окно не откроется (а в Linux часть из них
    // приводит к аварийному завершению), поэтому они проверяются заранее.
    if let Err(message) = platform::check_environment() {
        startup::show_error(&message);
        std::process::exit(1);
    }

    let icon = load_app_icon();
    let make_options = || {
        let mut viewport = egui::ViewportBuilder::default()
            .with_inner_size([1120.0, 720.0])
            .with_min_inner_size([420.0, 480.0])
            .with_title(platform::APP_NAME)
            .with_app_id(platform::APP_ID)
            .with_decorations(false)
            .with_transparent(true);
        if let Some(icon) = icon.clone() {
            viewport = viewport.with_icon(icon);
        }
        eframe::NativeOptions {
            viewport,
            persist_window: false,
            persistence_path: platform::settings_file(),
            // Цикл событий сохраняется между запусками, поэтому при неудаче
            // одного способа отрисовки можно сразу попробовать следующий.
            run_and_return: true,
            ..Default::default()
        }
    };

    // Способы отрисовки перебираются по порядку: сначала GPU, затем CPU.
    let mut renderer_errors = Vec::new();
    for attempt in renderer::attempts() {
        let mut options = make_options();
        renderer::apply(attempt, &mut options);
        renderer::set_current(attempt);
        // Некоторые библиотеки отрисовки сообщают об отсутствии подходящего GPU
        // паникой, а не ошибкой, поэтому паника во время запуска перехватывается
        // и считается неудачей этого способа отрисовки.
        startup::begin_renderer_start();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            eframe::run_native(
                platform::APP_NAME,
                options,
                Box::new(|cc| Ok(Box::new(App::new(cc)))),
            )
        }));
        let result = match result {
            Ok(result) => result,
            Err(_) if !startup::is_renderer_starting() => {
                // Паника после запуска — настоящий сбой: сообщение уже показано.
                std::process::exit(1);
            }
            Err(payload) => {
                let message = startup::panic_message(payload.as_ref());
                eprintln!("Отрисовка ({}) недоступна: {message}", attempt.describe());
                renderer_errors.push(format!("{}: {message}", attempt.describe()));
                continue;
            }
        };
        startup::renderer_started();
        match result {
            Err(err) if renderer::is_renderer_error(&err) => {
                eprintln!("Отрисовка ({}) недоступна: {err}", attempt.describe());
                renderer_errors.push(format!("{}: {err}", attempt.describe()));
            }
            Err(err) => {
                startup::show_error(&startup::compose("startup_failed", &err.to_string()));
                return Err(err);
            }
            Ok(()) if renderer::take_software_gpu_rejection() => {
                let reason = "вместо GPU запустилась программная отрисовка";
                eprintln!("Отрисовка ({}) недоступна: {reason}", attempt.describe());
                renderer_errors.push(format!("{}: {reason}", attempt.describe()));
            }
            Ok(()) => return Ok(()),
        }
    }

    // Ни один способ отрисовки не запустился.
    let details = renderer_errors.join("\n");
    startup::show_error(&startup::compose("startup_no_renderer", &details));
    std::process::exit(1);
}
