// Локализация интерфейса: язык приложения и таблица переводов.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Lang {
    En,
    Ru,
    Ja,
}

impl Lang {
    pub const ALL: [Lang; 3] = [Lang::En, Lang::Ru, Lang::Ja];

    pub fn label(&self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::Ru => "Русский",
            Lang::Ja => "日本語",
        }
    }
}

// Язык по умолчанию для нового запуска приложения.
impl Default for Lang {
    fn default() -> Self {
        Lang::En
    }
}

// Возвращает перевод строки `key` для языка `lang`.
// Если перевод не найден — возвращает пустую строку.
pub fn t(lang: Lang, key: &str) -> &'static str {
    match (lang, key) {
        (Lang::En, "subtitle") => "DXF Overlay Viewer",
        (Lang::Ru, "subtitle") => "Просмотр наложения DXF",
        (Lang::Ja, "subtitle") => "DXF 重ね合わせビューア",

        (Lang::En, "file_a") => "File A",
        (Lang::Ru, "file_a") => "Файл A",
        (Lang::Ja, "file_a") => "ファイル A",

        (Lang::En, "file_b") => "File B",
        (Lang::Ru, "file_b") => "Файл B",
        (Lang::Ja, "file_b") => "ファイル B",

        (Lang::En, "open_file") => "Open File...",
        (Lang::Ru, "open_file") => "Открыть файл...",
        (Lang::Ja, "open_file") => "ファイルを開く...",

        (Lang::En, "no_file") => "No file selected",
        (Lang::Ru, "no_file") => "Файл не выбран",
        (Lang::Ja, "no_file") => "ファイルが選択されていません",

        (Lang::En, "visible") => "Visible",
        (Lang::Ru, "visible") => "Показать",
        (Lang::Ja, "visible") => "表示する",

        (Lang::En, "opacity") => "Opacity",
        (Lang::Ru, "opacity") => "Прозрачность",
        (Lang::Ja, "opacity") => "不透明度",

        (Lang::En, "color") => "Color",
        (Lang::Ru, "color") => "Цвет",
        (Lang::Ja, "color") => "表示色",

        (Lang::En, "reset_settings") => "Reset Settings",
        (Lang::Ru, "reset_settings") => "Сбросить настройки",
        (Lang::Ja, "reset_settings") => "設定をリセット",

        (Lang::En, "fit") => "Fit to View",
        (Lang::Ru, "fit") => "Вписать в окно",
        (Lang::Ja, "fit") => "全体表示",

        (Lang::En, "entities") => "Entities",
        (Lang::Ru, "entities") => "Объекты",
        (Lang::Ja, "entities") => "図形数",

        (Lang::En, "scale_ratio") => "Scale Ratio",
        (Lang::Ru, "scale_ratio") => "Масштаб (соотношение)",
        (Lang::Ja, "scale_ratio") => "表示倍率（比率調整）",

        (Lang::En, "scale_reset") => "Reset Scale",
        (Lang::Ru, "scale_reset") => "Сбросить масштаб",
        (Lang::Ja, "scale_reset") => "倍率をリセット",

        (Lang::En, "tip") => "Tip: drag to pan, scroll or pinch to zoom around your cursor.",
        (Lang::Ru, "tip") => {
            "Совет: перетащите для перемещения, прокрутите/щипок для масштабирования у курсора."
        }
        (Lang::Ja, "tip") => {
            "ヒント：ドラッグで移動、スクロール/ピンチでカーソル位置を中心に拡大縮小できます。"
        }

        (Lang::En, "diff_title") => "Diff Highlight",
        (Lang::Ru, "diff_title") => "Подсветка различий",
        (Lang::Ja, "diff_title") => "差分ハイライト",

        (Lang::En, "diff_enable") => "Highlight non-overlapping parts",
        (Lang::Ru, "diff_enable") => "Подсвечивать несовпадающие участки",
        (Lang::Ja, "diff_enable") => "重ならない部分をハイライト表示する",

        (Lang::En, "diff_color_a") => "File A Diff Color",
        (Lang::Ru, "diff_color_a") => "Цвет различий A",
        (Lang::Ja, "diff_color_a") => "Aの差分色",

        (Lang::En, "diff_color_b") => "File B Diff Color",
        (Lang::Ru, "diff_color_b") => "Цвет различий B",
        (Lang::Ja, "diff_color_b") => "Bの差分色",

        (Lang::En, "diff_color_reset") => "Reset Color",
        (Lang::Ru, "diff_color_reset") => "Сбросить цвет",
        (Lang::Ja, "diff_color_reset") => "色をリセット",

        (Lang::En, "diff_tip") => {
            "Compares all supported 2D CAD elements — lines, curves (arcs/ellipses/splines approximated as polylines), circles, points, and text. Only parts that completely coincide count as identical; everything else is highlighted as a difference. File A's and File B's diffs are shown in separate colors."
        }
        (Lang::Ru, "diff_tip") => {
            "Сравнивает все поддерживаемые 2D CAD-элементы — линии, кривые (дуги/эллипсы/сплайны, аппроксимированные ломаными), окружности, точки и текст. Идентичными считаются только полностью совпадающие участки, всё остальное подсвечивается как различие. Различия файлов A и B показаны разными цветами."
        }
        (Lang::Ja, "diff_tip") => {
            "線分・曲線（円弧/楕円/スプライン等の折れ線近似を含む）・円・点・文字など、対応するすべての2D CAD要素を対象に、完全に重なっている部分だけを「同一」とみなし、それ以外はすべて差分としてハイライトします。Aの差分とBの差分は別の色で表示されます。"
        }

        (Lang::En, "diff_need_both") => "Load both File A and File B to see the diff highlight.",
        (Lang::Ru, "diff_need_both") => {
            "Чтобы увидеть подсветку различий, загрузите оба файла — A и B."
        }
        (Lang::Ja, "diff_need_both") => "差分表示には、AとBの両方のファイルを読み込んでください。",

        (Lang::En, "load_error") => "Load error",
        (Lang::Ru, "load_error") => "Ошибка загрузки",
        (Lang::Ja, "load_error") => "読み込みエラー",

        (Lang::En, "error_ok") => "OK",
        (Lang::Ru, "error_ok") => "ОК",
        (Lang::Ja, "error_ok") => "OK",

        (Lang::En, "encoding_dialog_title") => "Select Text Encoding",
        (Lang::Ru, "encoding_dialog_title") => "Выберите кодировку текста",
        (Lang::Ja, "encoding_dialog_title") => "文字コードを選択してください",

        (Lang::En, "encoding_dialog_desc") => {
            "This DXF file's text encoding could not be detected automatically. Please choose the encoding that displays the text correctly."
        }
        (Lang::Ru, "encoding_dialog_desc") => {
            "Не удалось автоматически определить кодировку текста этого DXF-файла. Выберите кодировку, при которой текст отображается корректно."
        }
        (Lang::Ja, "encoding_dialog_desc") => {
            "このDXFファイルの文字コードを自動判定できませんでした。文字が正しく表示される文字コードを下から選んでください。"
        }

        (Lang::En, "encoding_ok") => "Load with this encoding",
        (Lang::Ru, "encoding_ok") => "Загрузить с этой кодировкой",
        (Lang::Ja, "encoding_ok") => "この文字コードで読み込む",

        (Lang::En, "encoding_cancel") => "Cancel",
        (Lang::Ru, "encoding_cancel") => "Отмена",
        (Lang::Ja, "encoding_cancel") => "キャンセル",

        (Lang::En, "encoding_label") => "Encoding",
        (Lang::Ru, "encoding_label") => "Кодировка",
        (Lang::Ja, "encoding_label") => "文字コード",

        (Lang::En, "encoding_auto") => "Auto-detect",
        (Lang::Ru, "encoding_auto") => "Автоопределение",
        (Lang::Ja, "encoding_auto") => "自動判定",

        (Lang::En, "encoding_desc_western_european") => "Western European",
        (Lang::Ru, "encoding_desc_western_european") => "западноевропейские языки",
        (Lang::Ja, "encoding_desc_western_european") => "西欧",

        (Lang::En, "encoding_desc_cyrillic") => "Cyrillic",
        (Lang::Ru, "encoding_desc_cyrillic") => "кириллица",
        (Lang::Ja, "encoding_desc_cyrillic") => "キリル文字",

        (Lang::En, "encoding_desc_japanese") => "Japanese",
        (Lang::Ru, "encoding_desc_japanese") => "японский",
        (Lang::Ja, "encoding_desc_japanese") => "日本語",

        (Lang::En, "encoding_desc_simplified_chinese") => "Simplified Chinese",
        (Lang::Ru, "encoding_desc_simplified_chinese") => "упрощённый китайский",
        (Lang::Ja, "encoding_desc_simplified_chinese") => "簡体字中国語",

        (Lang::En, "encoding_desc_traditional_chinese") => "Traditional Chinese",
        (Lang::Ru, "encoding_desc_traditional_chinese") => "традиционный китайский",
        (Lang::Ja, "encoding_desc_traditional_chinese") => "繁体字中国語",

        (Lang::En, "encoding_desc_korean") => "Korean",
        (Lang::Ru, "encoding_desc_korean") => "корейский",
        (Lang::Ja, "encoding_desc_korean") => "韓国語",

        (Lang::En, "empty_state") => "Open two DXF files using the panel on the left",
        (Lang::Ru, "empty_state") => "Откройте два DXF-файла на панели слева",
        (Lang::Ja, "empty_state") => "「ファイルを開く」から DXF ファイルを2つ読み込んでください",

        (Lang::En, "select_file_title") => "Please select a DXF file",
        (Lang::Ru, "select_file_title") => "Выберите файл DXF",
        (Lang::Ja, "select_file_title") => "DXFファイルを選んでください",

        (Lang::En, "license_button_tooltip") => "License Information",
        (Lang::Ru, "license_button_tooltip") => "Информация о лицензиях",
        (Lang::Ja, "license_button_tooltip") => "ライセンス情報",

        (Lang::En, "license_title") => "Library & Font Licenses",
        (Lang::Ru, "license_title") => "Лицензии библиотек и шрифтов",
        (Lang::Ja, "license_title") => "使用ライブラリ・フォントのライセンス",

        (Lang::En, "license_libraries") => "Libraries Used",
        (Lang::Ru, "license_libraries") => "Используемые библиотеки",
        (Lang::Ja, "license_libraries") => "使用ライブラリ",

        (Lang::En, "license_fonts") => "Fonts Used",
        (Lang::Ru, "license_fonts") => "Используемые шрифты",
        (Lang::Ja, "license_fonts") => "使用フォント",

        (Lang::En, "license_fonts_body") => {
            "Google Sans and Noto Sans JP are both provided by Google LLC under the SIL Open Font License, Version 1.1 (OFL)."
        }
        (Lang::Ru, "license_fonts_body") => {
            "Google Sans и Noto Sans JP предоставляются Google LLC по лицензии SIL Open Font License, версия 1.1 (OFL)."
        }
        (Lang::Ja, "license_fonts_body") => {
            "Google Sans と Noto Sans JP は、いずれも SIL Open Font License, Version 1.1（OFL）のもとで Google LLC により提供されています。"
        }

        (Lang::En, "license_software") => "About This Software",
        (Lang::Ru, "license_software") => "Об этом программном обеспечении",
        (Lang::Ja, "license_software") => "本ソフトウェアについて",

        (Lang::En, "license_software_body") => {
            "This software is released under the MIT License. It is created by Rafych, and the source code is distributed at the repository below:"
        }
        (Lang::Ru, "license_software_body") => {
            "Это программное обеспечение распространяется по лицензии MIT. Автор — Rafych. Исходный код распространяется в следующем репозитории:"
        }
        (Lang::Ja, "license_software_body") => {
            "本ソフトウェアは MIT License のもとで公開されています。作者は Rafych です。ソースコードは以下のリポジトリで配布されています:"
        }

        (Lang::En, "picker_title") => "Open DXF File",
        (Lang::Ru, "picker_title") => "Открыть файл DXF",
        (Lang::Ja, "picker_title") => "DXFファイルを開く",

        (Lang::En, "picker_note") => {
            "The system file dialog is not available, so the built-in file browser is used."
        }
        (Lang::Ru, "picker_note") => {
            "Системный диалог выбора файла недоступен, поэтому используется встроенный обзор файлов."
        }
        (Lang::Ja, "picker_note") => {
            "システムのファイル選択ダイアログを利用できないため、内蔵のファイルブラウザーを使用しています。"
        }

        (Lang::En, "picker_up") => "Up",
        (Lang::Ru, "picker_up") => "Вверх",
        (Lang::Ja, "picker_up") => "上へ",

        (Lang::En, "picker_home") => "Home",
        (Lang::Ru, "picker_home") => "Домой",
        (Lang::Ja, "picker_home") => "ホーム",

        (Lang::En, "picker_path_hint") => "Path to a folder or DXF file",
        (Lang::Ru, "picker_path_hint") => "Путь к папке или DXF-файлу",
        (Lang::Ja, "picker_path_hint") => "フォルダーまたはDXFファイルのパス",

        (Lang::En, "picker_open") => "Open",
        (Lang::Ru, "picker_open") => "Открыть",
        (Lang::Ja, "picker_open") => "開く",

        (Lang::En, "picker_cancel") => "Cancel",
        (Lang::Ru, "picker_cancel") => "Отмена",
        (Lang::Ja, "picker_cancel") => "キャンセル",

        (Lang::En, "picker_empty") => "No folders or DXF files here",
        (Lang::Ru, "picker_empty") => "Здесь нет папок и DXF-файлов",
        (Lang::Ja, "picker_empty") => "フォルダーやDXFファイルがありません",

        (Lang::En, "picker_read_error") => "This folder cannot be opened",
        (Lang::Ru, "picker_read_error") => "Не удаётся открыть эту папку",
        (Lang::Ja, "picker_read_error") => "このフォルダーを開けません",

        (Lang::En, "picker_not_found") => "No such folder or file",
        (Lang::Ru, "picker_not_found") => "Такой папки или файла нет",
        (Lang::Ja, "picker_not_found") => "そのフォルダーまたはファイルは存在しません",

        (Lang::En, "startup_missing_libs") => {
            "Pyatorka cannot start because required system libraries are missing:"
        }
        (Lang::Ru, "startup_missing_libs") => {
            "Пятёрка не может запуститься: отсутствуют необходимые системные библиотеки:"
        }
        (Lang::Ja, "startup_missing_libs") => {
            "必要なシステムライブラリが見つからないため、Пятёрка を起動できません:"
        }

        (Lang::En, "startup_install_hint") => {
            "Install them with your package manager. On Debian/Ubuntu:"
        }
        (Lang::Ru, "startup_install_hint") => {
            "Установите их через менеджер пакетов. В Debian/Ubuntu:"
        }
        (Lang::Ja, "startup_install_hint") => {
            "パッケージマネージャーでインストールしてください。Debian/Ubuntu の場合:"
        }

        (Lang::En, "startup_no_display") => {
            "Pyatorka cannot start because no graphical session was found (neither WAYLAND_DISPLAY nor DISPLAY is set)."
        }
        (Lang::Ru, "startup_no_display") => {
            "Пятёрка не может запуститься: графический сеанс не найден (не заданы ни WAYLAND_DISPLAY, ни DISPLAY)."
        }
        (Lang::Ja, "startup_no_display") => {
            "グラフィカルセッションが見つからないため、Пятёрка を起動できません（WAYLAND_DISPLAY も DISPLAY も設定されていません）。"
        }

        (Lang::En, "startup_failed") => "Pyatorka could not start.",
        (Lang::Ru, "startup_failed") => "Пятёрка не смогла запуститься.",
        (Lang::Ja, "startup_failed") => "Пятёрка を起動できませんでした。",

        (Lang::En, "startup_crashed") => "Pyatorka stopped because of an unexpected error.",
        (Lang::Ru, "startup_crashed") => "Пятёрка остановлена из-за непредвиденной ошибки.",
        (Lang::Ja, "startup_crashed") => "予期しないエラーが発生したため、Пятёрка を終了しました。",

        (Lang::En, "license_fonts_egui") => "Fonts bundled with the egui library:",
        (Lang::Ru, "license_fonts_egui") => "Шрифты, входящие в библиотеку egui:",
        (Lang::Ja, "license_fonts_egui") => "egui ライブラリに付属するフォント:",

        (Lang::En, "license_full_texts") => {
            "Full license texts of all included libraries and fonts are in THIRD_PARTY_LICENSES.html (included with the download):"
        }
        (Lang::Ru, "license_full_texts") => {
            "Полные тексты лицензий всех входящих библиотек и шрифтов — в файле THIRD_PARTY_LICENSES.html (входит в загрузку):"
        }
        (Lang::Ja, "license_full_texts") => {
            "含まれるすべてのライブラリとフォントのライセンス全文は THIRD_PARTY_LICENSES.html（ダウンロードに同梱）にあります:"
        }

        (Lang::En, "startup_no_renderer") => {
            "Pyatorka could not start because neither the GPU nor CPU (software) rendering could be initialized."
        }
        (Lang::Ru, "startup_no_renderer") => {
            "Пятёрка не смогла запуститься: не удалось включить отрисовку ни на GPU, ни на CPU (программную)."
        }
        (Lang::Ja, "startup_no_renderer") => {
            "GPU でも CPU（ソフトウェア）でも描画を初期化できなかったため、Пятёрка を起動できませんでした。"
        }

        (Lang::En, "render_title") => "Rendering",
        (Lang::Ru, "render_title") => "Отрисовка",
        (Lang::Ja, "render_title") => "描画",

        (Lang::En, "render_gpu") => "GPU (hardware acceleration)",
        (Lang::Ru, "render_gpu") => "GPU (аппаратное ускорение)",
        (Lang::Ja, "render_gpu") => "GPU（ハードウェアアクセラレーション）",

        (Lang::En, "render_cpu") => "CPU (software rendering — no usable GPU was found)",
        (Lang::Ru, "render_cpu") => "CPU (программная отрисовка — подходящий GPU не найден)",
        (Lang::Ja, "render_cpu") => {
            "CPU（ソフトウェア描画 — 使用できる GPU が見つかりませんでした）"
        }

        _ => "",
    }
}
