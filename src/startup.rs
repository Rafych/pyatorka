// Проверки при запуске и понятные сообщения об ошибках вместо аварийного завершения.
use crate::i18n::{Lang, t};
use std::sync::atomic::{AtomicBool, Ordering};

// Собирает текст сообщения: заголовок на всех языках интерфейса (en → ru → ja)
// и, при наличии, технические подробности. Язык из настроек при запуске
// ещё не известен, поэтому сообщение выводится сразу на всех языках.
pub fn compose(key: &str, details: &str) -> String {
    let mut text = Lang::ALL
        .iter()
        .map(|&lang| t(lang, key))
        .collect::<Vec<_>>()
        .join("\n");
    if !details.is_empty() {
        text.push_str("\n\n");
        text.push_str(details);
    }
    text
}

// Показывает сообщение об ошибке: в стандартный поток ошибок и в отдельном окне
// (в Windows — системное окно сообщения, в Linux — через zenity, если он установлен).
pub fn show_error(message: &str) {
    eprintln!("{message}");
    let _ = rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Error)
        .set_title(crate::platform::APP_NAME)
        .set_description(message)
        .set_buttons(rfd::MessageButtons::Ok)
        .show();
}

// Идёт ли сейчас запуск отрисовки. Паника в это время (некоторые библиотеки
// отрисовки так сообщают об отсутствии подходящего GPU) — не сбой приложения:
// main перехватывает её и пробует следующий способ отрисовки.
static RENDERER_STARTING: AtomicBool = AtomicBool::new(false);

// Отмечает начало запуска очередного способа отрисовки.
pub fn begin_renderer_start() {
    RENDERER_STARTING.store(true, Ordering::SeqCst);
}

// Отмечает, что отрисовка запустилась и приложение работает (первый кадр).
pub fn renderer_started() {
    RENDERER_STARTING.store(false, Ordering::SeqCst);
}

// Запускалась ли отрисовка в момент паники (то есть паника — ошибка запуска отрисовки).
pub fn is_renderer_starting() -> bool {
    RENDERER_STARTING.load(Ordering::SeqCst)
}

// Текст паники для сообщений и журнала.
pub fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|s| s.to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_default()
}

// Устанавливает обработчик паники: вместо молчаливого аварийного завершения
// пользователь видит сообщение с причиной, после чего приложение закрывается.
// Паники во время запуска отрисовки только записываются в журнал.
pub fn install_panic_hook() {
    static SHOWN: AtomicBool = AtomicBool::new(false);
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        default_hook(info);
        if is_renderer_starting() {
            return;
        }
        // Повторная паника (например, внутри самого окна сообщения) не должна
        // приводить к бесконечной цепочке окон.
        if SHOWN.swap(true, Ordering::SeqCst) {
            return;
        }
        let payload = panic_message(info.payload());
        let details = match info.location() {
            Some(location) => format!("{payload}\n({}:{})", location.file(), location.line()),
            None => payload,
        };
        show_error(&compose("startup_crashed", &details));
    }));
}
