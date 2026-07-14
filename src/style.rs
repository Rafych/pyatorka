// Модуль оформления: светлая тема в духе macOS/Apple (мягкий серый фон,
// скруглённые углы, приглушённая насыщенность, фирменный синий акцент).
use eframe::egui;
use egui::{Color32, Rounding, Stroke, Visuals};

pub const ACCENT: Color32 = Color32::from_rgb(0, 122, 255);
pub const COLOR_A_DEFAULT: Color32 = Color32::from_rgb(0, 122, 255);
pub const COLOR_B_DEFAULT: Color32 = Color32::from_rgb(255, 69, 58);

// Цвета подсветки различий по умолчанию: не пересекаются с цветами A/B и хорошо
// привлекают внимание. Файл A — жёлтый, файл B — зелёный.
pub const DIFF_COLOR_A_DEFAULT: Color32 = Color32::from_rgb(255, 204, 0);
pub const DIFF_COLOR_B_DEFAULT: Color32 = Color32::from_rgb(52, 199, 89);

// Константы шрифтов (FONT_JA / FONT_RU / FONT_EN), сгенерированные build.rs.
include!(concat!(env!("OUT_DIR"), "/fonts_generated.rs"));

// Применяет тему и настройки стиля ко всему приложению.
pub fn apply(ctx: &egui::Context) {
    let mut visuals = Visuals::light();

    let rounding = Rounding::same(10.0);
    visuals.window_rounding = Rounding::same(14.0);
    visuals.menu_rounding = rounding;
    visuals.widgets.noninteractive.rounding = rounding;
    visuals.widgets.inactive.rounding = rounding;
    visuals.widgets.hovered.rounding = rounding;
    visuals.widgets.active.rounding = rounding;
    visuals.widgets.open.rounding = rounding;

    visuals.window_fill = Color32::from_rgb(246, 246, 248);
    visuals.panel_fill = Color32::from_rgb(246, 246, 248);
    visuals.extreme_bg_color = Color32::from_rgb(255, 255, 255);
    visuals.faint_bg_color = Color32::from_rgb(238, 238, 241);

    visuals.widgets.inactive.bg_fill = Color32::from_rgb(233, 233, 237);
    visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(233, 233, 237);
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(220, 232, 255);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT.linear_multiply(0.6));
    visuals.widgets.active.bg_fill = ACCENT;
    // Цвет текста в состоянии "active" намеренно не переопределяется на белый:
    // это затронуло бы, например, подписи включённых чекбоксов, и текст стал бы
    // нечитаемым на светлом фоне.

    visuals.selection.bg_fill = ACCENT;
    visuals.selection.stroke = Stroke::new(1.0, Color32::WHITE);
    visuals.hyperlink_color = ACCENT;

    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(228, 228, 232));
    visuals.window_stroke = Stroke::new(1.0, Color32::from_rgb(228, 228, 232));
    visuals.window_shadow = egui::epaint::Shadow {
        offset: egui::vec2(0.0, 10.0),
        blur: 32.0,
        spread: 0.0,
        color: Color32::from_black_alpha(55),
    };
    visuals.popup_shadow = egui::epaint::Shadow {
        offset: egui::vec2(0.0, 6.0),
        blur: 18.0,
        spread: 0.0,
        color: Color32::from_black_alpha(45),
    };

    ctx.set_visuals(visuals);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(10.0, 10.0);
    style.spacing.button_padding = egui::vec2(14.0, 7.0);
    style.spacing.window_margin = egui::Margin::same(14.0);
    style.spacing.indent = 16.0;
    style.animation_time = 0.12;

    // Подписи (кнопки, заголовки, описания) нельзя выделять протяжкой мыши;
    // поля ввода (egui::TextEdit) это ограничение не затрагивает.
    style.interaction.selectable_labels = false;
    ctx.set_style(style);

    setup_fonts(ctx);
}

// Подключает шрифты с поддержкой японских иероглифов и кириллицы, чтобы
// интерфейс корректно отображался на любом из трёх языков (JA/RU/EN)
// независимо от текущего выбранного языка.
fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    fonts
        .font_data
        .insert("font_ja".to_owned(), egui::FontData::from_static(FONT_JA));
    fonts
        .font_data
        .insert("font_ru".to_owned(), egui::FontData::from_static(FONT_RU));
    fonts
        .font_data
        .insert("font_en".to_owned(), egui::FontData::from_static(FONT_EN));

    // egui выбирает для каждого символа первый шрифт из списка, где есть
    // нужный глиф, поэтому порядок регистрации значения не имеет — важно
    // лишь, чтобы все три шрифта были подключены одновременно.
    let proportional = fonts
        .families
        .get_mut(&egui::FontFamily::Proportional)
        .unwrap();
    proportional.insert(0, "font_ru".to_owned());
    proportional.insert(1, "font_en".to_owned());
    proportional.insert(2, "font_ja".to_owned());

    let monospace = fonts
        .families
        .get_mut(&egui::FontFamily::Monospace)
        .unwrap();
    monospace.push("font_ru".to_owned());
    monospace.push("font_en".to_owned());
    monospace.push("font_ja".to_owned());

    ctx.set_fonts(fonts);
}
