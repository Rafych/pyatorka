// Встроенный обзор файлов — запасной вариант на случай, когда системный диалог
// выбора файла недоступен (например, в Linux без xdg-desktop-portal и zenity).
use crate::i18n::{Lang, t};
use eframe::egui;
use egui::{Color32, Vec2};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

// Взводится, когда rfd сообщил в журнал, что не смог показать диалог.
static NATIVE_DIALOG_FAILED: AtomicBool = AtomicBool::new(false);

// Журнал, перехватывающий сообщения rfd. Если системный диалог недоступен,
// rfd возвращает тот же результат, что и при отмене пользователем, и лишь
// пишет в журнал об ошибке — по ней и определяется, что нужен запасной вариант.
struct DialogLogger;

impl log::Log for DialogLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::Level::Warn && metadata.target().starts_with("rfd")
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let message = record.args().to_string();
        // Сначала rfd пробует xdg-desktop-portal, затем zenity. Неудачей считается
        // только провал последней попытки — иначе отмена в окне zenity была бы
        // ошибочно принята за недоступность диалога.
        if record.level() == log::Level::Error && message.contains("zenity") {
            NATIVE_DIALOG_FAILED.store(true, Ordering::Relaxed);
        }
        eprintln!("[{}] {}: {}", record.level(), record.target(), message);
    }

    fn flush(&self) {}
}

// Подключает перехват сообщений rfd. Вызывается один раз при запуске.
pub fn install_dialog_logger() {
    if log::set_boxed_logger(Box::new(DialogLogger)).is_ok() {
        log::set_max_level(log::LevelFilter::Warn);
    }
}

// Сбрасывает признак неудачи перед открытием системного диалога.
pub fn reset_native_dialog_failure() {
    NATIVE_DIALOG_FAILED.store(false, Ordering::Relaxed);
}

// Сообщил ли rfd с момента сброса, что системный диалог показать не удалось.
pub fn native_dialog_failed() -> bool {
    NATIVE_DIALOG_FAILED.load(Ordering::Relaxed)
}

fn is_dxf(path: &Path) -> bool {
    path.extension()
        .map(|e| e.eq_ignore_ascii_case("dxf"))
        .unwrap_or(false)
}

// Элемент списка: вложенная папка или DXF-файл.
struct Entry {
    name: String,
    path: PathBuf,
    is_dir: bool,
}

// Состояние окна встроенного обзора файлов.
pub struct FilePicker {
    pub slot_index: usize,
    pub forced_encoding: Option<&'static encoding_rs::Encoding>,
    dir: PathBuf,
    path_input: String,
    entries: Vec<Entry>,
    selected: Option<usize>,
    // Ключ локализации сообщения об ошибке (папка не читается, путь не найден).
    error_key: Option<&'static str>,
}

// Что выбрал пользователь в окне обзора за текущий кадр.
pub enum PickerAction {
    None,
    Cancel,
    Open(PathBuf),
}

impl FilePicker {
    pub fn new(
        slot_index: usize,
        forced_encoding: Option<&'static encoding_rs::Encoding>,
        start_dir: Option<PathBuf>,
    ) -> Self {
        let mut picker = Self {
            slot_index,
            forced_encoding,
            dir: PathBuf::new(),
            path_input: String::new(),
            entries: Vec::new(),
            selected: None,
            error_key: None,
        };
        let dir = start_dir
            .filter(|d| d.is_dir())
            .unwrap_or_else(crate::platform::home_dir);
        picker.navigate(dir);
        picker
    }

    // Переходит в папку `dir` и перечитывает её содержимое: сначала папки,
    // затем DXF-файлы, по алфавиту без учёта регистра. Скрытые элементы не показываются.
    fn navigate(&mut self, dir: PathBuf) {
        self.selected = None;
        self.error_key = None;
        self.entries.clear();
        match std::fs::read_dir(&dir) {
            Ok(read_dir) => {
                for item in read_dir.flatten() {
                    let name = item.file_name().to_string_lossy().into_owned();
                    if name.starts_with('.') {
                        continue;
                    }
                    let path = item.path();
                    let is_dir = path.is_dir();
                    if is_dir || is_dxf(&path) {
                        self.entries.push(Entry { name, path, is_dir });
                    }
                }
                self.entries.sort_by(|a, b| {
                    b.is_dir
                        .cmp(&a.is_dir)
                        .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
                });
            }
            Err(_) => self.error_key = Some("picker_read_error"),
        }
        self.path_input = dir.display().to_string();
        self.dir = dir;
    }

    // Обрабатывает путь, введённый вручную: папка — открыть, DXF-файл — выбрать.
    fn submit_path(&mut self) -> Option<PathBuf> {
        let path = PathBuf::from(self.path_input.trim());
        if path.is_dir() {
            self.navigate(path);
            None
        } else if path.is_file() {
            Some(path)
        } else {
            self.error_key = Some("picker_not_found");
            None
        }
    }

    // Файл, который будет открыт кнопкой «Открыть».
    fn chosen_file(&self) -> Option<PathBuf> {
        if let Some(entry) = self.selected.and_then(|i| self.entries.get(i)) {
            return (!entry.is_dir).then(|| entry.path.clone());
        }
        let typed = PathBuf::from(self.path_input.trim());
        typed.is_file().then_some(typed)
    }
}

// Показывает окно встроенного обзора файлов и возвращает действие пользователя.
pub fn show(picker: &mut FilePicker, ctx: &egui::Context, lang: Lang) -> PickerAction {
    let mut navigate_to: Option<PathBuf> = None;
    let mut open: Option<PathBuf> = None;
    let mut cancel = false;

    let closed = crate::modal_window(
        ctx,
        "file_picker",
        t(lang, "picker_title"),
        Vec2::new(560.0, 480.0),
        |ui| {
            ui.label(egui::RichText::new(t(lang, "picker_note")).small().weak());
            ui.add_space(4.0);

            ui.horizontal(|ui| {
                if ui.button(t(lang, "picker_up")).clicked() {
                    navigate_to = picker.dir.parent().map(Path::to_path_buf);
                }
                if ui.button(t(lang, "picker_home")).clicked() {
                    navigate_to = Some(crate::platform::home_dir());
                }
                let response = ui.add(
                    egui::TextEdit::singleline(&mut picker.path_input)
                        .hint_text(t(lang, "picker_path_hint"))
                        .desired_width(ui.available_width()),
                );
                if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    open = picker.submit_path();
                }
            });

            if let Some(key) = picker.error_key {
                ui.colored_label(Color32::from_rgb(209, 40, 30), t(lang, key));
            }

            const FOOTER_HEIGHT: f32 = 32.0;
            let list_height =
                (ui.available_height() - FOOTER_HEIGHT - ui.spacing().item_spacing.y).max(0.0);
            egui::Frame::new()
                .fill(Color32::WHITE)
                .corner_radius(egui::CornerRadius::same(8))
                .stroke(egui::Stroke::new(1.0, Color32::from_rgb(230, 230, 235)))
                .inner_margin(egui::Margin::same(5))
                .show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .max_height((list_height - 12.0).max(0.0))
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            // Строки списка растягиваются на всю ширину, текст — по левому краю.
                            ui.with_layout(
                                egui::Layout::top_down_justified(egui::Align::Min),
                                |ui| {
                                    ui.spacing_mut().item_spacing.y = 2.0;
                                    ui.spacing_mut().button_padding = egui::vec2(10.0, 4.0);
                                    if picker.entries.is_empty() && picker.error_key.is_none() {
                                        ui.label(
                                            egui::RichText::new(t(lang, "picker_empty")).weak(),
                                        );
                                    }
                                    for (i, entry) in picker.entries.iter().enumerate() {
                                        // Знак «›» есть во встроенных шрифтах (в отличие от «▸»).
                                        let text = if entry.is_dir {
                                            format!("›  {}/", entry.name)
                                        } else {
                                            format!("     {}", entry.name)
                                        };
                                        let response =
                                            ui.selectable_label(picker.selected == Some(i), text);
                                        if entry.is_dir {
                                            if response.clicked() {
                                                navigate_to = Some(entry.path.clone());
                                            }
                                        } else if response.double_clicked() {
                                            open = Some(entry.path.clone());
                                        } else if response.clicked() {
                                            picker.selected = Some(i);
                                        }
                                    }
                                },
                            );
                        });
                });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let chosen = picker.chosen_file();
                let open_button = egui::Button::new(
                    egui::RichText::new(t(lang, "picker_open")).color(Color32::WHITE),
                )
                .fill(crate::style::ACCENT)
                .corner_radius(egui::CornerRadius::same(8));
                if ui
                    .add_enabled_ui(chosen.is_some(), |ui| {
                        ui.add_sized([96.0, FOOTER_HEIGHT], open_button)
                    })
                    .inner
                    .clicked()
                {
                    open = chosen;
                }
                if ui
                    .add_sized(
                        [96.0, FOOTER_HEIGHT],
                        egui::Button::new(t(lang, "picker_cancel")),
                    )
                    .clicked()
                {
                    cancel = true;
                }
            });
        },
    );

    if let Some(dir) = navigate_to {
        picker.navigate(dir);
    }
    if let Some(path) = open {
        return PickerAction::Open(path);
    }
    if closed || cancel {
        return PickerAction::Cancel;
    }
    PickerAction::None
}
