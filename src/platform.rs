// Всё, что зависит от операционной системы, собрано в этом модуле.
// Остальной код приложения одинаков для Windows и Linux и обращается только
// к функциям ниже, а различия между системами скрыты в модулях `imp`.
use std::path::PathBuf;

// Имя приложения: заголовок окна и папка с сохранёнными настройками.
pub const APP_NAME: &str = "Пятёрка";

// Идентификатор приложения для оконной системы Linux (app_id в Wayland,
// класс окна в X11): по нему окружение рабочего стола связывает окно с ярлыком.
pub const APP_ID: &str = "pyatorka";

// Проверяет перед открытием окна, что в системе есть всё необходимое.
// Возвращает готовый текст сообщения об ошибке.
pub fn check_environment() -> Result<(), String> {
    imp::check_environment()
}

// Дополнительная настройка окна после его создания (вызывается один раз).
pub fn configure_window(frame: &eframe::Frame) {
    imp::configure_window(frame);
}

// Файл с сохранёнными настройками. Путь задаётся явно, чтобы он не зависел
// от APP_ID и совпадал с прежним расположением настроек.
pub fn settings_file() -> Option<PathBuf> {
    eframe::storage_dir(APP_NAME).map(|dir| dir.join("app.ron"))
}

// Домашняя папка пользователя (или текущая папка, если домашнюю определить нельзя).
pub fn home_dir() -> PathBuf {
    std::env::var_os(imp::HOME_VAR)
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("/"))
}

#[cfg(target_os = "windows")]
mod imp {
    pub const HOME_VAR: &str = "USERPROFILE";

    // В Windows всё необходимое входит в состав системы.
    pub fn check_environment() -> Result<(), String> {
        Ok(())
    }

    // Отключает системную тень и рамку Windows, которые eframe включает для окон
    // без рамки: окно рисует собственные скруглённые углы и обводку.
    pub fn configure_window(frame: &eframe::Frame) {
        use winit::platform::windows::WindowExtWindows as _;
        if let Some(window) = frame.winit_window() {
            window.set_undecorated_shadow(false);
        }
    }
}

#[cfg(target_os = "linux")]
mod imp {
    use crate::i18n::{Lang, t};
    use crate::startup::compose;

    pub const HOME_VAR: &str = "HOME";

    // Системные библиотеки, без которых окно не откроется. Каждый элемент —
    // взаимозаменяемые имена файлов библиотеки и пакет Debian/Ubuntu с ней.
    type Requirement = (&'static [&'static str], &'static str);

    const WAYLAND_LIBRARIES: &[Requirement] = &[
        (
            &["libwayland-client.so.0", "libwayland-client.so"],
            "libwayland-client0",
        ),
        (
            &["libwayland-egl.so.1", "libwayland-egl.so"],
            "libwayland-egl1",
        ),
        (&["libxkbcommon.so.0", "libxkbcommon.so"], "libxkbcommon0"),
        (&["libEGL.so.1", "libEGL.so"], "libegl1"),
    ];

    const X11_LIBRARIES: &[Requirement] = &[
        (&["libX11.so.6", "libX11.so"], "libx11-6"),
        (&["libX11-xcb.so.1", "libX11-xcb.so"], "libx11-xcb1"),
        (&["libxcb.so.1", "libxcb.so"], "libxcb1"),
        (&["libXcursor.so.1", "libXcursor.so"], "libxcursor1"),
        (&["libXi.so.6", "libXi.so"], "libxi6"),
        (&["libxkbcommon.so.0", "libxkbcommon.so"], "libxkbcommon0"),
        (
            &["libxkbcommon-x11.so.0", "libxkbcommon-x11.so"],
            "libxkbcommon-x11-0",
        ),
        // Для OpenGL подходит либо EGL, либо GLX.
        (
            &["libEGL.so.1", "libGL.so.1", "libEGL.so", "libGL.so"],
            "libegl1",
        ),
    ];

    // Можно ли загрузить библиотеку `name` (так же, как это делают winit и glutin).
    fn can_load(name: &str) -> bool {
        let Ok(c_name) = std::ffi::CString::new(name) else {
            return false;
        };
        // SAFETY: dlopen/dlclose вызываются с корректной C-строкой; полученный
        // дескриптор сразу же закрывается и больше не используется.
        unsafe {
            let handle = libc::dlopen(c_name.as_ptr(), libc::RTLD_LAZY | libc::RTLD_LOCAL);
            if handle.is_null() {
                false
            } else {
                libc::dlclose(handle);
                true
            }
        }
    }

    // Проверяет, что есть графический сеанс и все библиотеки для него.
    // Бэкенд выбирается так же, как в winit: Wayland, если задан WAYLAND_DISPLAY
    // (или WAYLAND_SOCKET), иначе X11.
    pub fn check_environment() -> Result<(), String> {
        let is_set = |name: &str| std::env::var(name).map(|v| !v.is_empty()).unwrap_or(false);
        let required = if is_set("WAYLAND_DISPLAY") || is_set("WAYLAND_SOCKET") {
            WAYLAND_LIBRARIES
        } else if is_set("DISPLAY") {
            X11_LIBRARIES
        } else {
            return Err(compose("startup_no_display", ""));
        };

        let missing: Vec<&Requirement> = required
            .iter()
            .filter(|(names, _)| !names.iter().any(|name| can_load(name)))
            .collect();
        if missing.is_empty() {
            return Ok(());
        }

        let libraries = missing
            .iter()
            .map(|(names, _)| format!("  • {}", names[0]))
            .collect::<Vec<_>>()
            .join("\n");
        let packages = missing
            .iter()
            .map(|(_, package)| *package)
            .collect::<Vec<_>>()
            .join(" ");
        let install_hint = Lang::ALL
            .iter()
            .map(|&lang| t(lang, "startup_install_hint"))
            .collect::<Vec<_>>()
            .join("\n");
        Err(compose(
            "startup_missing_libs",
            &format!("{libraries}\n\n{install_hint}\n  sudo apt install {packages}"),
        ))
    }

    // В Linux окно не требует дополнительной настройки.
    pub fn configure_window(_frame: &eframe::Frame) {}
}

// Прочие системы: проверок и дополнительной настройки окна нет.
#[cfg(not(any(target_os = "windows", target_os = "linux")))]
mod imp {
    pub const HOME_VAR: &str = "HOME";

    pub fn check_environment() -> Result<(), String> {
        Ok(())
    }

    pub fn configure_window(_frame: &eframe::Frame) {}
}
