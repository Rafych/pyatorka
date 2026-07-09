#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// Точка входа приложения Пятёрка: окно, состояние, отрисовка и обработчики UI.
mod diff;
mod encoding;
mod geometry;
mod i18n;
mod style;

use eframe::egui;
use egui::{Color32, Pos2, Rect, Stroke, Vec2};
use geometry::{Geometry, TextStyle};
use i18n::{t, Lang};

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

// Формирует подпись варианта кодировки в диалоге выбора, например "Shift_JIS (Japanese)".
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
    gen: u64,
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
            gen: 0,
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
    error: Option<String>,
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
    scale_slider_dragging: bool,
    subtitle_width_cache: Option<(Lang, f32)>,
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
            scale_slider_dragging: false,
            subtitle_width_cache: None,
        }
    }
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        style::apply(&cc.egui_ctx);
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
                    self.slot_a.gen,
                    self.slot_b.gen,
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
    fn open_dialog(
        &mut self,
        slot_index: usize,
        forced_encoding: Option<&'static encoding_rs::Encoding>,
    ) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("DXF", &["dxf", "DXF"])
            .set_title(t(self.lang, "select_file_title"))
            .pick_file()
        {
            self.load_path_into_slot(path, slot_index, forced_encoding);
        }
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
                slot.gen = slot.gen.wrapping_add(1);
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
                self.error = Some(format!("{}: {}", t(self.lang, "load_error"), e));
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
    // модальные окна (лицензия, выбор кодировки) и полоса ошибки.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.scale_slider_dragging = false;

        if !self.show_license && self.pending_encoding.is_none() {
            handle_resize_edges(ctx);
        }

        self.custom_title_bar(ctx);

        self.license_window(ctx);
        self.encoding_choice_window(ctx);

        if let Some(err) = self.error.as_deref() {
            egui::TopBottomPanel::top("error_bar")
                .frame(egui::Frame::none().fill(Color32::from_rgb(250, 250, 251)))
                .show(ctx, |ui| {
                    ui.add_space(6.0);
                    egui::Frame::none()
                        .fill(Color32::from_rgb(255, 235, 234))
                        .stroke(Stroke::new(1.0, Color32::from_rgb(255, 200, 197)))
                        .rounding(egui::Rounding::same(10.0))
                        .inner_margin(egui::Margin::symmetric(12.0, 8.0))
                        .show(ui, |ui| {
                            ui.colored_label(Color32::from_rgb(209, 40, 30), err);
                        });
                    ui.add_space(6.0);
                });
        }

        self.update_diff_cache(ctx);

        // Ширина левой панели управления фиксирована.
        const PANEL_FIXED_WIDTH: f32 = 280.0;
        let panel_width = PANEL_FIXED_WIDTH;

        let side_frame = egui::Frame::none()
            .fill(Color32::from_rgb(246, 246, 248))
            .inner_margin(egui::Margin::same(14.0))
            .rounding(egui::Rounding {
                nw: 0.0,
                ne: 0.0,
                sw: 14.0,
                se: 0.0,
            })
            .stroke(Stroke::new(1.0, Color32::from_rgb(228, 228, 232)));

        egui::SidePanel::left("control_panel")
            .resizable(false)
            .exact_width(panel_width)
            .frame(side_frame)
            .show(ctx, |ui| {
                self.control_panel_body(ui);
            });

        let central_frame = egui::Frame::none()
            .fill(Color32::from_rgb(252, 252, 253))
            .rounding(egui::Rounding {
                nw: 0.0,
                ne: 0.0,
                sw: 0.0,
                se: 14.0,
            })
            .stroke(Stroke::new(1.0, Color32::from_rgb(228, 228, 232)));
        egui::CentralPanel::default()
            .frame(central_frame)
            .show(ctx, |ui| {
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

                let scroll = if cursor_in_canvas {
                    ui.input(|i| i.raw_scroll_delta.y)
                } else {
                    0.0
                };
                let pinch = if cursor_in_canvas {
                    ui.input(|i| i.zoom_delta())
                } else {
                    1.0
                };

                // Колесо мыши и пинч-жест объединяются в общий коэффициент масштабирования.
                let mut zoom_factor: f32 = 1.0;
                if scroll != 0.0 {
                    zoom_factor *= (1.0 + scroll * 0.0015_f32).clamp(0.5, 1.5);
                }
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
                    egui::Rounding {
                        nw: 0.0,
                        ne: 0.0,
                        sw: 0.0,
                        se: 14.0,
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
                            let col = slot.color.linear_multiply(slot.opacity);
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
                                painter.circle_stroke(sc, sr, stroke);
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
                            let glow_a = Stroke::new(6.0, self.diff_color_a.linear_multiply(0.30));
                            let core_a = Stroke::new(2.2, self.diff_color_a);
                            let glow_b = Stroke::new(6.0, self.diff_color_b.linear_multiply(0.30));
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
                                        painter.circle_stroke(sc, sr, glow_a);
                                    }
                                }
                            }
                            if self.diff_b_visible {
                                for (c, r) in &d.only_b_circles {
                                    let sc = to_screen_prescaled(*c);
                                    let sr = r * scale;
                                    if circle_visible(view, sc, sr) {
                                        painter.circle_stroke(sc, sr, glow_b);
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
                                        painter.circle_stroke(sc, sr, core_a);
                                    }
                                }
                            }
                            if self.diff_b_visible {
                                for (c, r) in &d.only_b_circles {
                                    let sc = to_screen_prescaled(*c);
                                    let sr = r * scale;
                                    if circle_visible(view, sc, sr) {
                                        painter.circle_stroke(sc, sr, core_b);
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
                                        self.diff_color_a.linear_multiply(0.30),
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
                                        self.diff_color_b.linear_multiply(0.30),
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
                                let galley =
                                    ui.fonts(|f| f.layout_no_wrap(s.clone(), font_id.clone(), col));
                                let text_rect = egui::Align2::LEFT_BOTTOM
                                    .anchor_rect(Rect::from_min_size(sp, galley.size()));
                                painter.rect_filled(
                                    text_rect.expand(2.0),
                                    egui::Rounding::same(3.0),
                                    col.linear_multiply(0.25),
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
    fn custom_title_bar(&mut self, ctx: &egui::Context) {
        let bar_height = 40.0;

        let frame = egui::Frame::none()
            .fill(Color32::from_rgb(250, 250, 251))
            .rounding(egui::Rounding {
                nw: 14.0,
                ne: 14.0,
                sw: 0.0,
                se: 0.0,
            })
            .stroke(Stroke::new(1.0, Color32::from_rgb(228, 228, 232)))
            .inner_margin(egui::Margin {
                left: 14.0,
                right: 4.0,
                top: 0.0,
                bottom: 0.0,
            });

        let title_bar_interactive = !self.show_license;

        egui::TopBottomPanel::top("title_bar")
            .exact_height(bar_height)
            .frame(frame)
            .show(ctx, |ui| {
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
                            ui.label(egui::RichText::new("Пятёрка").size(15.0).strong());

                            // Ширина подзаголовка кэшируется по языку, чтобы не пересчитывать
                            // раскладку текста (layout_no_wrap) на каждом кадре.
                            let subtitle_text = t(self.lang, "subtitle");
                            let subtitle_width = match self.subtitle_width_cache {
                                Some((lang, w)) if lang == self.lang => w,
                                _ => {
                                    let galley = ui.fonts(|f| {
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
                    ui.allocate_ui_at_rect(combo_rect, |ui| {
                        ui.style_mut().spacing.button_padding = egui::vec2(8.0, 4.0);
                        egui::ComboBox::from_id_source("lang_selector")
                            .width(combo_width)
                            .selected_text(self.lang.label())
                            .show_ui(ui, |ui| {
                                for l in Lang::ALL {
                                    ui.selectable_value(&mut self.lang, l, l.label());
                                }
                            });
                    });

                    let license_resp = ui
                        .allocate_ui_at_rect(license_rect, |ui| {
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
    }

    // Модальный диалог выбора кодировки текста, показывается, когда
    // автоопределение не смогло уверенно декодировать файл.
    fn encoding_choice_window(&mut self, ctx: &egui::Context) {
        let Some(pending) = &self.pending_encoding else {
            return;
        };
        let lang = self.lang;
        let screen = ctx.screen_rect();

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

                ui.allocate_ui_at_rect(win_rect, |ui| {
                    ui.set_width(window_size.x);
                    ui.set_height(window_size.y);

                    egui::Frame::none()
                        .fill(Color32::from_rgb(246, 246, 248))
                        .rounding(egui::Rounding::same(14.0))
                        .stroke(Stroke::new(1.0, Color32::from_rgb(228, 228, 232)))
                        .inner_margin(egui::Margin::same(18.0))
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

    // Модальное окно со сведениями о лицензиях используемых библиотек и шрифтов.
    fn license_window(&mut self, ctx: &egui::Context) {
        if !self.show_license {
            return;
        }

        let lang = self.lang;
        let screen = ctx.screen_rect();

        const MARGIN: f32 = 24.0;
        let window_size = Vec2::new(
            460.0_f32.min((screen.width() - MARGIN * 2.0).max(240.0)),
            480.0_f32.min((screen.height() - MARGIN * 2.0).max(260.0)),
        );

        let win_rect = Rect::from_center_size(screen.center(), window_size);
        let mut close_clicked = false;

        egui::Area::new(egui::Id::new("license_modal_layer"))
            .order(egui::Order::Foreground)
            .fixed_pos(Pos2::ZERO)
            .interactable(true)
            .show(ctx, |ui| {
                ui.allocate_response(screen.size(), egui::Sense::click_and_drag());
                ui.painter()
                    .rect_filled(screen, 0.0, Color32::from_black_alpha(90));

                ui.allocate_ui_at_rect(win_rect, |ui| {
                    ui.set_width(window_size.x);
                    ui.set_height(window_size.y);

                    egui::Frame::none()
                        .fill(Color32::from_rgb(246, 246, 248))
                        .rounding(egui::Rounding::same(14.0))
                        .stroke(Stroke::new(1.0, Color32::from_rgb(228, 228, 232)))
                        .show(ui, |ui| {
                            ui.set_width(window_size.x);
                            ui.set_height(window_size.y);

                            let bar_height = 36.0;

                            let bar_frame = egui::Frame::none()
                                .fill(Color32::from_rgb(250, 250, 251))
                                .rounding(egui::Rounding {
                                    nw: 14.0,
                                    ne: 14.0,
                                    sw: 0.0,
                                    se: 0.0,
                                });

                            egui::TopBottomPanel::top("license_title_bar")
                                .exact_height(bar_height)
                                .frame(bar_frame)
                                .show_inside(ui, |ui| {
                                    let bar_rect = ui.max_rect();

                                    ui.painter().text(
                                        bar_rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        t(lang, "license_title"),
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
                                        .allocate_ui_at_rect(close_rect, |ui| {
                                            close_x_button(ui, close_diameter)
                                        })
                                        .inner;
                                    if resp.clicked() {
                                        close_clicked = true;
                                    }
                                });

                            let body_rounding = egui::Rounding {
                                nw: 0.0,
                                ne: 0.0,
                                sw: 14.0,
                                se: 14.0,
                            };
                            let body_frame = egui::Frame::none()
                                .fill(Color32::from_rgb(246, 246, 248))
                                .rounding(body_rounding)
                                .inner_margin(egui::Margin::same(16.0));

                            egui::CentralPanel::default()
                                .frame(body_frame)
                                .show_inside(ui, |ui| {
                                    egui::ScrollArea::vertical()
                                        .auto_shrink([false, false])
                                        .scroll_bar_visibility(
                                            egui::scroll_area::ScrollBarVisibility::AlwaysHidden,
                                        )
                                        .show(ui, |ui| {
                                            ui.label(
                                                egui::RichText::new(t(lang, "license_libraries"))
                                                    .strong(),
                                            );
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

                                            ui.label(
                                                egui::RichText::new(t(lang, "license_fonts"))
                                                    .strong(),
                                            );
                                            ui.add_space(4.0);
                                            ui.label(t(lang, "license_fonts_body"));
                                            ui.add_space(6.0);
                                            ui.horizontal(|ui| {
                                                ui.label("•");
                                                ui.label(
                                            "Google Sans — SIL Open Font License 1.1 (Google LLC)",
                                        );
                                            });
                                            ui.horizontal(|ui| {
                                                ui.label("•");
                                                ui.label(
                                            "Noto Sans JP — SIL Open Font License 1.1 (Google LLC)",
                                        );
                                            });
                                            ui.add_space(4.0);
                                            ui.hyperlink_to(
                                                "openfontlicense.org",
                                                "https://openfontlicense.org",
                                            );

                                            ui.add_space(14.0);
                                            ui.separator();
                                            ui.add_space(10.0);

                                            ui.label(
                                                egui::RichText::new(t(lang, "license_software"))
                                                    .strong(),
                                            );
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

                                            ui.add_space(14.0);
                                        });
                                });
                        });
                });
            });

        if close_clicked || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.show_license = false;
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

        egui::Frame::none()
            .fill(Color32::from_rgb(255, 255, 255))
            .rounding(egui::Rounding::same(10.0))
            .stroke(Stroke::new(1.0, Color32::from_rgb(230, 230, 235)))
            .inner_margin(egui::Margin::same(12.0))
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
                        .rounding(egui::Rounding::same(8.0)),
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

                egui::ComboBox::from_id_source(("encoding_combo", idx))
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
        egui::Frame::none()
            .fill(Color32::from_rgb(255, 255, 255))
            .rounding(egui::Rounding::same(10.0))
            .stroke(Stroke::new(1.0, Color32::from_rgb(230, 230, 235)))
            .inner_margin(egui::Margin::same(12.0))
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

// Реализует изменение размера окна без системной рамки: определяет,
// когда курсор находится у края окна, и запускает системный resize-drag.
fn handle_resize_edges(ctx: &egui::Context) {
    let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
    if maximized {
        return;
    }

    let screen = ctx.screen_rect();
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
            ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::ResizeEast);
        }
        if south.hovered() || south.dragged() {
            ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::ResizeSouth);
        }
        if corner.hovered() || corner.dragged() {
            ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::ResizeSouthEast);
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
    painter.circle_stroke(center, r - 1.0, Stroke::new(1.4, color));

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
                painter.rect_stroke(Rect::from_center_size(c, Vec2::splat(r * 2.0)), 1.0, stroke);
            }
        }
    }
    response.clicked()
}

// Список сторонних библиотек и их лицензий для окна "О программе".
const LIBRARY_LICENSES: &[(&str, &str, &str)] = &[
    ("eframe", "0.27", "MIT OR Apache-2.0"),
    ("egui", "0.27", "MIT OR Apache-2.0"),
    ("dxf", "0.5", "MIT"),
    ("rfd", "0.14", "MIT"),
    ("serde", "1", "MIT OR Apache-2.0"),
    ("image", "0.24", "MIT"),
    ("winres", "0.1", "MIT (только Windows)"),
];

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
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([1120.0, 720.0])
        .with_min_inner_size([420.0, 480.0])
        .with_title("Пятёрка")
        .with_decorations(false)
        .with_transparent(true);

    if let Some(icon) = load_app_icon() {
        viewport = viewport.with_icon(icon);
    }

    let options = eframe::NativeOptions {
        viewport,
        persist_window: false,
        ..Default::default()
    };

    eframe::run_native("Пятёрка", options, Box::new(|cc| Box::new(App::new(cc))))
}
