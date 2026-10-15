// Сборочный скрипт: подготавливает встраиваемые в бинарник шрифты и иконку.
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

// Находит файл шрифта для языка `lang_code` (assets/fonts/<lang_code>.ttf).
// Если такого файла нет, используется общий assets/fonts/general.ttf.
// Если нет и его — сборка завершается с ошибкой (это ожидаемое поведение).
fn resolve_font_path(lang_code: &str) -> PathBuf {
    let specific = format!("assets/fonts/{}.ttf", lang_code);
    let general = "assets/fonts/general.ttf";

    let source = if Path::new(&specific).exists() {
        specific
    } else if Path::new(general).exists() {
        println!(
            "cargo:warning=Файл assets/fonts/{}.ttf не найден, вместо него используется assets/fonts/general.ttf.",
            lang_code
        );
        general.to_string()
    } else {
        panic!(
            "Файл шрифта не найден: не существует ни assets/fonts/{}.ttf, ни assets/fonts/general.ttf.\n\
             Поместите один из этих файлов в assets/fonts/ и повторите сборку.",
            lang_code
        );
    };

    println!("cargo:rerun-if-changed={}", source);

    let canonical = fs::canonicalize(&source).unwrap_or_else(|e| {
        panic!(
            "Не удалось определить абсолютный путь к файлу шрифта ({}): {}",
            source, e
        )
    });

    strip_unc_prefix(canonical)
}

// Убирает Windows-префикс \\?\ у абсолютного пути, чтобы include_bytes!
// корректно воспринимал путь на всех платформах.
fn strip_unc_prefix(path: PathBuf) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(stripped) = s.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else {
        path
    }
}

// Экранирует обратные слэши пути для вставки в исходный код как строковый литерал.
fn path_literal(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "\\\\")
}

// Генерирует fonts_generated.rs с константами FONT_EN / FONT_RU / FONT_JA.
// Если несколько языков используют один и тот же файл (например, все три
// попадают на general.ttf), файл встраивается только один раз —
// остальные константы лишь ссылаются на уже встроенные байты.
fn generate_font_bindings(out_dir: &str) {
    let langs = ["en", "ru", "ja"];
    let paths: Vec<PathBuf> = langs.iter().map(|l| resolve_font_path(l)).collect();

    let mut unique_paths: Vec<PathBuf> = Vec::new();
    for p in &paths {
        if !unique_paths.contains(p) {
            unique_paths.push(p.clone());
        }
    }

    let mut code = String::new();
    for (i, p) in unique_paths.iter().enumerate() {
        code.push_str(&format!(
            "static GEN_FONT_{}: &[u8] = include_bytes!(\"{}\");\n",
            i,
            path_literal(p)
        ));
    }
    code.push('\n');

    let const_names = ["FONT_EN", "FONT_RU", "FONT_JA"];
    for (lang_path, const_name) in paths.iter().zip(const_names.iter()) {
        let idx = unique_paths.iter().position(|p| p == lang_path).unwrap();
        code.push_str(&format!(
            "pub static {}: &[u8] = GEN_FONT_{};\n",
            const_name, idx
        ));
    }

    let dest = Path::new(out_dir).join("fonts_generated.rs");
    fs::write(&dest, code)
        .unwrap_or_else(|e| panic!("Не удалось записать fonts_generated.rs: {}", e));
}

// Готовит PNG-иконку приложения (для окна). Если assets/icon.png отсутствует,
// подставляется прозрачная заглушка 1x1, чтобы сборка не падала.
fn resolve_icon(out_dir: &str) {
    let icon_src = "assets/icon.png";
    let icon_dest = Path::new(out_dir).join("icon.png");
    let flag_dest = Path::new(out_dir).join("icon_info.rs");

    if Path::new(icon_src).exists() {
        fs::copy(icon_src, &icon_dest)
            .unwrap_or_else(|e| panic!("Не удалось скопировать icon.png: {}", e));
        fs::write(&flag_dest, "pub const HAS_ICON: bool = true;\n")
            .unwrap_or_else(|e| panic!("Не удалось записать icon_info.rs: {}", e));
        println!("cargo:rerun-if-changed={}", icon_src);
    } else {
        const DUMMY_PNG_1X1_TRANSPARENT: &[u8] = &[
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00,
            0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78,
            0x9C, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00,
            0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
        ];
        fs::write(&icon_dest, DUMMY_PNG_1X1_TRANSPARENT)
            .unwrap_or_else(|e| panic!("Не удалось записать заглушку icon.png: {}", e));
        fs::write(&flag_dest, "pub const HAS_ICON: bool = false;\n")
            .unwrap_or_else(|e| panic!("Не удалось записать icon_info.rs: {}", e));
        println!(
            "cargo:warning=Файл assets/icon.png не найден, значок окна останется стандартным."
        );
    }
}

// ---------------------------------------------------------------------------
// Ресурсы Windows (значок .exe и сведения о версии).
//
// Файл ресурсов (.res) собирается здесь же, на чистом Rust, и передаётся
// компоновщику напрямую — без компилятора ресурсов rc.exe и сторонних крейтов.
// Код одинаково работает при сборке и на Windows, и на Linux.
// ---------------------------------------------------------------------------

const RT_ICON: u16 = 3;
const RT_GROUP_ICON: u16 = 14;
const RT_VERSION: u16 = 16;
// Флаги памяти, которые ставит и rc.exe: MOVEABLE | PURE (| DISCARDABLE для значков).
const MEMFLAGS_ICON: u16 = 0x1030;
const MEMFLAGS_VERSION: u16 = 0x0030;
// Язык, не зависящий от региона (так же, как раньше делал winres).
const LANG_NEUTRAL: u16 = 0x0000;
const CODEPAGE_UNICODE: u16 = 0x04B0;

fn push_u16(buf: &mut Vec<u8>, v: u16) {
    buf.extend_from_slice(&v.to_le_bytes());
}

fn push_u32(buf: &mut Vec<u8>, v: u32) {
    buf.extend_from_slice(&v.to_le_bytes());
}

// Строка UTF-16LE с завершающим нулём.
fn push_utf16z(buf: &mut Vec<u8>, s: &str) {
    for unit in s.encode_utf16() {
        push_u16(buf, unit);
    }
    push_u16(buf, 0);
}

// Выравнивание до границы 4 байт, обязательное в формате .res.
fn pad4(buf: &mut Vec<u8>) {
    while !buf.len().is_multiple_of(4) {
        buf.push(0);
    }
}

// Добавляет в файл .res одну запись с числовыми типом и именем.
fn push_resource(res: &mut Vec<u8>, type_id: u16, name_id: u16, mem_flags: u16, data: &[u8]) {
    push_u32(res, data.len() as u32); // DataSize
    push_u32(res, 32); // HeaderSize
    push_u16(res, 0xFFFF);
    push_u16(res, type_id);
    push_u16(res, 0xFFFF);
    push_u16(res, name_id);
    push_u32(res, 0); // DataVersion
    push_u16(res, mem_flags);
    push_u16(res, LANG_NEUTRAL);
    push_u32(res, 0); // Version
    push_u32(res, 0); // Characteristics
    res.extend_from_slice(data);
    pad4(res);
}

// Значение узла в дереве сведений о версии (VS_VERSIONINFO).
enum VersionValue<'a> {
    None,
    Binary(&'a [u8]),
    Text(&'a str),
}

// Собирает узел VS_VERSIONINFO: заголовок, ключ, значение и вложенные узлы.
// `text_type` задаёт wType: 1 — текстовые узлы и контейнеры, 0 — двоичные.
fn version_node(key: &str, value: VersionValue, text_type: bool, children: &[Vec<u8>]) -> Vec<u8> {
    let mut node = vec![0u8; 6];
    push_utf16z(&mut node, key);
    pad4(&mut node);
    let value_length = match value {
        VersionValue::None => 0,
        VersionValue::Binary(bytes) => {
            node.extend_from_slice(bytes);
            bytes.len()
        }
        VersionValue::Text(text) => {
            push_utf16z(&mut node, text);
            // Для текста длина значения указывается в символах UTF-16 с нулём.
            text.encode_utf16().count() + 1
        }
    };
    for child in children {
        pad4(&mut node);
        node.extend_from_slice(child);
    }
    let length = node.len() as u16;
    node[0..2].copy_from_slice(&length.to_le_bytes());
    node[2..4].copy_from_slice(&(value_length as u16).to_le_bytes());
    node[4..6].copy_from_slice(&(text_type as u16).to_le_bytes());
    node
}

// Сведения о версии, видимые в свойствах .exe (те же поля, что ставил winres).
fn version_info_resource() -> Vec<u8> {
    let part = |name: &str| -> u32 {
        env::var(name)
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0)
    };
    let (major, minor, patch) = (
        part("CARGO_PKG_VERSION_MAJOR"),
        part("CARGO_PKG_VERSION_MINOR"),
        part("CARGO_PKG_VERSION_PATCH"),
    );
    let version = env::var("CARGO_PKG_VERSION").unwrap_or_default();
    let name = env::var("CARGO_PKG_NAME").unwrap_or_default();

    // VS_FIXEDFILEINFO.
    let mut fixed = Vec::new();
    push_u32(&mut fixed, 0xFEEF04BD); // dwSignature
    push_u32(&mut fixed, 0x0001_0000); // dwStrucVersion
    push_u32(&mut fixed, (major << 16) | minor); // dwFileVersionMS
    push_u32(&mut fixed, patch << 16); // dwFileVersionLS
    push_u32(&mut fixed, (major << 16) | minor); // dwProductVersionMS
    push_u32(&mut fixed, patch << 16); // dwProductVersionLS
    push_u32(&mut fixed, 0x3F); // dwFileFlagsMask
    push_u32(&mut fixed, 0); // dwFileFlags
    push_u32(&mut fixed, 0x0004_0004); // dwFileOS = VOS_NT_WINDOWS32
    push_u32(&mut fixed, 1); // dwFileType = VFT_APP
    push_u32(&mut fixed, 0); // dwFileSubtype
    push_u32(&mut fixed, 0); // dwFileDateMS
    push_u32(&mut fixed, 0); // dwFileDateLS

    let strings: Vec<Vec<u8>> = [
        ("FileDescription", name.as_str()),
        ("FileVersion", version.as_str()),
        ("ProductName", name.as_str()),
        ("ProductVersion", version.as_str()),
    ]
    .iter()
    .map(|(key, value)| version_node(key, VersionValue::Text(value), true, &[]))
    .collect();
    let table_key = format!("{:04x}{:04x}", LANG_NEUTRAL, CODEPAGE_UNICODE);
    let string_table = version_node(&table_key, VersionValue::None, true, &strings);
    let string_file_info =
        version_node("StringFileInfo", VersionValue::None, true, &[string_table]);

    let mut translation = Vec::new();
    push_u16(&mut translation, LANG_NEUTRAL);
    push_u16(&mut translation, CODEPAGE_UNICODE);
    let var = version_node(
        "Translation",
        VersionValue::Binary(&translation),
        false,
        &[],
    );
    let var_file_info = version_node("VarFileInfo", VersionValue::None, true, &[var]);

    version_node(
        "VS_VERSION_INFO",
        VersionValue::Binary(&fixed),
        false,
        &[string_file_info, var_file_info],
    )
}

// Разбирает файл .ico и добавляет в .res каждое изображение (RT_ICON)
// и общий каталог значков (RT_GROUP_ICON) с номером 1.
fn push_icon_resources(res: &mut Vec<u8>, ico: &[u8]) -> Result<(), String> {
    const BROKEN: &str = "файл значка повреждён";
    let read_u16 = |at: usize| {
        ico.get(at..at + 2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
    };
    let read_u32 = |at: usize| {
        ico.get(at..at + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };
    if read_u16(0) != Some(0) || read_u16(2) != Some(1) {
        return Err("это не файл значка (.ico)".to_string());
    }
    let count = read_u16(4).ok_or(BROKEN)?;

    let mut group = Vec::new();
    push_u16(&mut group, 0); // idReserved
    push_u16(&mut group, 1); // idType = значок
    push_u16(&mut group, count);
    for i in 0..count {
        let entry = 6 + 16 * i as usize;
        let header = ico.get(entry..entry + 8).ok_or(BROKEN)?;
        let size = read_u32(entry + 8).ok_or(BROKEN)? as usize;
        let offset = read_u32(entry + 12).ok_or(BROKEN)? as usize;
        let image = ico.get(offset..offset + size).ok_or(BROKEN)?;
        let icon_id = i + 1;
        push_resource(res, RT_ICON, icon_id, MEMFLAGS_ICON, image);
        // GRPICONDIRENTRY: первые 8 байт совпадают с записью .ico,
        // затем идут размер данных и номер ресурса RT_ICON.
        group.extend_from_slice(header);
        push_u32(&mut group, size as u32);
        push_u16(&mut group, icon_id);
    }
    push_resource(res, RT_GROUP_ICON, 1, MEMFLAGS_ICON, &group);
    Ok(())
}

// Собирает файл ресурсов Windows и подключает его к компоновке исполняемого файла.
fn embed_windows_resources(out_dir: &str) {
    // Готовый .res принимает компоновщик MSVC (link.exe). Для цели windows-gnu
    // нужен другой формат, поэтому там ресурсы не встраиваются.
    if env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("msvc") {
        println!(
            "cargo:warning=Ресурсы Windows встраиваются только для цели MSVC; значок .exe останется стандартным."
        );
        return;
    }

    // Пустая запись в начале — обязательный признак файла .res.
    let mut res = Vec::new();
    push_resource(&mut res, 0, 0, 0, &[]);

    let ico_path = "assets/icon.ico";
    println!("cargo:rerun-if-changed={}", ico_path);
    match fs::read(ico_path) {
        Ok(ico) => {
            if let Err(e) = push_icon_resources(&mut res, &ico) {
                panic!("Не удалось встроить значок Windows ({}): {}", ico_path, e);
            }
        }
        Err(_) => println!(
            "cargo:warning=Файл assets/icon.ico не найден, значок .exe останется стандартным."
        ),
    }

    push_resource(
        &mut res,
        RT_VERSION,
        1,
        MEMFLAGS_VERSION,
        &version_info_resource(),
    );

    let res_path = Path::new(out_dir).join("pyatorka.res");
    fs::write(&res_path, &res)
        .unwrap_or_else(|e| panic!("Не удалось записать файл ресурсов: {}", e));
    println!("cargo:rustc-link-arg-bins={}", res_path.display());
}

fn main() {
    let out_dir = env::var("OUT_DIR").expect("Переменная окружения OUT_DIR не задана");

    generate_font_bindings(&out_dir);

    println!("cargo:rerun-if-changed=assets/fonts/general.ttf");
    println!("cargo:rerun-if-changed=assets/fonts");

    resolve_icon(&out_dir);

    // Ресурсы .exe нужны только при сборке под Windows. Проверяется целевая ОС
    // (а не ОС, на которой идёт сборка), поэтому кросс-сборка работает в обе стороны.
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_windows_resources(&out_dir);
    }
}
