// Модуль оформления: светлая тема в духе macOS/Apple (мягкий серый фон,
// скруглённые углы, приглушённая насыщенность, фирменный синий акцент).
use eframe::egui;
use egui::{Color32, CornerRadius, Stroke, Visuals};

pub const ACCENT: Color32 = Color32::from_rgb(0, 122, 255);
pub const COLOR_A_DEFAULT: Color32 = Color32::from_rgb(0, 122, 255);
pub const COLOR_B_DEFAULT: Color32 = Color32::from_rgb(255, 69, 58);

// Цвета подсветки различий по умолчанию: не пересекаются с цветами A/B и хорошо
// привлекают внимание. Файл A — жёлтый, файл B — зелёный.
pub const DIFF_COLOR_A_DEFAULT: Color32 = Color32::from_rgb(255, 204, 0);
pub const DIFF_COLOR_B_DEFAULT: Color32 = Color32::from_rgb(52, 199, 89);

// Делает цвет полупрозрачным умножением в линейном пространстве. Каналы
// результата переводятся в гамма-пространство напрямую (без деления на альфу),
// поэтому полупрозрачные цвета получаются светлыми и насыщенными — так выглядят
// подсветка различий, прозрачность чертежей и т.п.
pub fn linear_multiply(color: Color32, factor: f32) -> Color32 {
    use egui::ecolor::{
        gamma_u8_from_linear_f32, linear_f32_from_gamma_u8, linear_u8_from_linear_f32,
    };
    let [r, g, b, a] = color.to_array();
    let channel = |c: u8| gamma_u8_from_linear_f32(linear_f32_from_gamma_u8(c) * factor);
    Color32::from_rgba_premultiplied(
        channel(r),
        channel(g),
        channel(b),
        linear_u8_from_linear_f32(a as f32 / 255.0 * factor),
    )
}

// Константы шрифтов (FONT_EN / FONT_RU / FONT_JA), сгенерированные build.rs.
include!(concat!(env!("OUT_DIR"), "/fonts_generated.rs"));

// Применяет тему и настройки стиля ко всему приложению.
pub fn apply(ctx: &egui::Context) {
    let mut visuals = Visuals::light();

    let rounding = CornerRadius::same(10);
    visuals.window_corner_radius = CornerRadius::same(14);
    visuals.menu_corner_radius = rounding;
    visuals.widgets.noninteractive.corner_radius = rounding;
    visuals.widgets.inactive.corner_radius = rounding;
    visuals.widgets.hovered.corner_radius = rounding;
    visuals.widgets.active.corner_radius = rounding;
    visuals.widgets.open.corner_radius = rounding;

    visuals.window_fill = Color32::from_rgb(246, 246, 248);
    visuals.panel_fill = Color32::from_rgb(246, 246, 248);
    visuals.extreme_bg_color = Color32::from_rgb(255, 255, 255);
    visuals.faint_bg_color = Color32::from_rgb(238, 238, 241);

    visuals.widgets.inactive.bg_fill = Color32::from_rgb(233, 233, 237);
    visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(233, 233, 237);
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(220, 232, 255);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, linear_multiply(ACCENT, 0.6));
    visuals.widgets.active.bg_fill = ACCENT;
    // Кнопки слегка «вырастают» при наведении и нажатии.
    visuals.widgets.hovered.expansion = 1.0;
    visuals.widgets.active.expansion = 1.0;
    // Цвет текста в состоянии "active" намеренно не переопределяется на белый:
    // это затронуло бы, например, подписи включённых чекбоксов, и текст стал бы
    // нечитаемым на светлом фоне.

    visuals.selection.bg_fill = ACCENT;
    visuals.selection.stroke = Stroke::new(1.0, Color32::WHITE);
    visuals.hyperlink_color = ACCENT;

    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(228, 228, 232));
    visuals.window_stroke = Stroke::new(1.0, Color32::from_rgb(228, 228, 232));
    // Второстепенный текст — непрозрачный серый посередине между цветом текста и фоном.
    visuals.weak_text_color = Some(visuals.gray_out(visuals.text_color()));
    // Круглый маркер у ползунков.
    visuals.handle_shape = egui::style::HandleShape::Circle;
    visuals.window_shadow = egui::epaint::Shadow {
        offset: [0, 10],
        blur: 32,
        spread: 0,
        color: Color32::from_black_alpha(55),
    };
    visuals.popup_shadow = egui::epaint::Shadow {
        offset: [0, 6],
        blur: 18,
        spread: 0,
        color: Color32::from_black_alpha(45),
    };

    // Приложение всегда использует собственную светлую тему, независимо
    // от тёмного/светлого режима системы.
    let mut style = (*ctx.style_of(egui::Theme::Light)).clone();
    style.visuals = visuals;
    style.spacing.item_spacing = egui::vec2(10.0, 10.0);
    style.spacing.button_padding = egui::vec2(14.0, 7.0);
    style.spacing.window_margin = egui::Margin::same(14);
    style.spacing.indent = 16.0;
    style.spacing.tooltip_width = 600.0;
    style.spacing.menu_width = 150.0;
    style.interaction.tooltip_delay = 0.3;

    // Размеры шрифтов интерфейса.
    use egui::FontFamily::{Monospace, Proportional};
    use egui::{FontId, TextStyle};
    style.text_styles = [
        (TextStyle::Small, FontId::new(9.0, Proportional)),
        (TextStyle::Body, FontId::new(12.5, Proportional)),
        (TextStyle::Button, FontId::new(12.5, Proportional)),
        (TextStyle::Heading, FontId::new(18.0, Proportional)),
        (TextStyle::Monospace, FontId::new(12.0, Monospace)),
    ]
    .into();
    style.animation_time = 0.12;

    // Подписи (кнопки, заголовки, описания) нельзя выделять протяжкой мыши;
    // поля ввода (egui::TextEdit) это ограничение не затрагивает.
    style.interaction.selectable_labels = false;
    ctx.set_style_of(egui::Theme::Light, style.clone());
    ctx.set_style_of(egui::Theme::Dark, style);
    ctx.set_theme(egui::Theme::Light);

    setup_fonts(ctx);
}

// Подключает шрифты с поддержкой латиницы, кириллицы и японских иероглифов, чтобы
// интерфейс корректно отображался на любом из трёх языков (EN/RU/JA)
// независимо от текущего выбранного языка.
fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    fonts.font_data.insert(
        "font_en".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(FONT_EN)),
    );
    fonts.font_data.insert(
        "font_ru".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(FONT_RU)),
    );
    fonts.font_data.insert(
        "font_ja".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(FONT_JA)),
    );

    // egui выбирает для каждого символа первый шрифт из списка, где есть
    // нужный глиф. Шрифты подключаются в порядке en → ru → ja: латиница
    // берётся из английского шрифта, кириллица — из русского, а японские
    // иероглифы — из японского.
    let proportional = fonts
        .families
        .get_mut(&egui::FontFamily::Proportional)
        .unwrap();
    proportional.insert(0, "font_en".to_owned());
    proportional.insert(1, "font_ru".to_owned());
    proportional.insert(2, "font_ja".to_owned());

    let monospace = fonts
        .families
        .get_mut(&egui::FontFamily::Monospace)
        .unwrap();
    monospace.push("font_en".to_owned());
    monospace.push("font_ru".to_owned());
    monospace.push("font_ja".to_owned());

    ctx.set_fonts(fonts);
}
