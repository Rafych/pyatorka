// Определение кодировки текста DXF-файла и декодирование в UTF-8.
use encoding_rs::Encoding;

#[derive(Clone)]
pub struct DetectedEncoding {
    pub encoding: &'static Encoding,
    pub label: String,
    pub had_bom: bool,
}

// Результат попытки автоматического определения кодировки.
pub enum DetectOutcome {
    // Кодировка определена уверенно, текст уже декодирован.
    Detected {
        text: String,
        info: DetectedEncoding,
    },
    // Автоматика не справилась — нужно спросить пользователя.
    // guess_index указывает на предполагаемый вариант в SELECTABLE_ENCODINGS,
    // если такое предположение вообще удалось сделать.
    NeedsUserChoice {
        guess_index: Option<usize>,
    },
}

// Список кодировок, которые пользователь может выбрать вручную в диалоге.
// Третий элемент кортежа — ключ локализации для короткого описания языка
// (пустая строка, если описание не требуется).
pub const SELECTABLE_ENCODINGS: &[(&str, &'static Encoding, &str)] = &[
    ("UTF-8", encoding_rs::UTF_8, ""),
    ("UTF-16LE", encoding_rs::UTF_16LE, ""),
    ("UTF-16BE", encoding_rs::UTF_16BE, ""),
    (
        "Shift_JIS",
        encoding_rs::SHIFT_JIS,
        "encoding_desc_japanese",
    ),
    ("EUC-JP", encoding_rs::EUC_JP, "encoding_desc_japanese"),
    ("GBK", encoding_rs::GBK, "encoding_desc_simplified_chinese"),
    (
        "Big5",
        encoding_rs::BIG5,
        "encoding_desc_traditional_chinese",
    ),
    ("EUC-KR", encoding_rs::EUC_KR, "encoding_desc_korean"),
    (
        "Windows-1251",
        encoding_rs::WINDOWS_1251,
        "encoding_desc_cyrillic",
    ),
    (
        "Windows-1252",
        encoding_rs::WINDOWS_1252,
        "encoding_desc_western_european",
    ),
];

// Находит индекс кодировки в SELECTABLE_ENCODINGS, чтобы можно было заранее
// выделить в диалоге вариант, предложенный автоопределением.
fn selectable_index_of(enc: &'static Encoding) -> Option<usize> {
    SELECTABLE_ENCODINGS.iter().position(|(_, e, _)| *e == enc)
}

// Пытается определить кодировку байтов файла и декодировать их в текст.
// Порядок проверки: 1) BOM, 2) строгий UTF-8 без BOM, 3) эвристика chardetng.
// Если ни один способ не дал результата без ошибок декодирования —
// возвращается NeedsUserChoice.
pub fn detect_and_decode(bytes: &[u8]) -> DetectOutcome {
    if let Some((enc, bom_len)) = Encoding::for_bom(bytes) {
        let (cow, _, had_errors) = enc.decode(bytes);
        if !had_errors {
            return DetectOutcome::Detected {
                text: cow.into_owned(),
                info: DetectedEncoding {
                    encoding: enc,
                    label: format!("{} (BOM)", enc.name()),
                    had_bom: bom_len > 0,
                },
            };
        }
    }

    if let Ok(s) = std::str::from_utf8(bytes) {
        return DetectOutcome::Detected {
            text: s.to_string(),
            info: DetectedEncoding {
                encoding: encoding_rs::UTF_8,
                label: "UTF-8 (auto)".to_string(),
                had_bom: false,
            },
        };
    }

    let mut detector = chardetng::EncodingDetector::new();
    detector.feed(bytes, true);
    let guessed = detector.guess(None, true);

    let (cow, _, had_errors) = guessed.decode(bytes);
    if !had_errors {
        DetectOutcome::Detected {
            text: cow.into_owned(),
            info: DetectedEncoding {
                encoding: guessed,
                label: format!("{} (auto)", guessed.name()),
                had_bom: false,
            },
        }
    } else {
        DetectOutcome::NeedsUserChoice {
            guess_index: selectable_index_of(guessed),
        }
    }
}

// Принудительно декодирует байты в заданной кодировке (ошибки декодирования
// игнорируются) — используется, когда пользователь выбрал кодировку вручную.
pub fn decode_with(bytes: &[u8], enc: &'static Encoding) -> String {
    let (cow, _, _had_errors) = enc.decode(bytes);
    cow.into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_utf8_bom() {
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice("SECTION".as_bytes());
        match detect_and_decode(&bytes) {
            DetectOutcome::Detected { text, info } => {
                assert_eq!(text, "SECTION");
                assert_eq!(info.encoding, encoding_rs::UTF_8);
                assert!(info.had_bom);
            }
            DetectOutcome::NeedsUserChoice { .. } => panic!("expected Detected"),
        }
    }

    #[test]
    fn detects_strict_utf8_without_bom() {
        let bytes = "Привет DXF".as_bytes();
        match detect_and_decode(bytes) {
            DetectOutcome::Detected { text, info } => {
                assert_eq!(text, "Привет DXF");
                assert_eq!(info.encoding, encoding_rs::UTF_8);
                assert!(!info.had_bom);
            }
            DetectOutcome::NeedsUserChoice { .. } => panic!("expected Detected"),
        }
    }

    #[test]
    fn falls_back_to_chardetng_for_shift_jis() {
        let (bytes, _, had_errors) = encoding_rs::SHIFT_JIS.encode("図面のテスト用テキストです");
        assert!(!had_errors);
        assert!(std::str::from_utf8(&bytes).is_err());
        match detect_and_decode(&bytes) {
            DetectOutcome::Detected { text, .. } => {
                assert_eq!(text, "図面のテスト用テキストです");
            }
            DetectOutcome::NeedsUserChoice { .. } => {}
        }
    }

    #[test]
    fn decode_with_forces_decoding_ignoring_errors() {
        let bytes = [0xFFu8, 0xFEu8, 0x00, 0x01];
        let text = decode_with(&bytes, encoding_rs::WINDOWS_1252);
        assert!(!text.is_empty());
    }
}
