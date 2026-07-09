// Загрузка DXF-файла и преобразование его содержимого в плоский набор
// примитивов (отрезки, окружности, точки, надписи), готовый для отрисовки.
use egui::Pos2;
use std::io::Cursor;
use std::path::Path;

// Раскрывает escape-последовательности вида \U+XXXX / \u+XXXX, которыми
// в DXF (до AutoCAD 2007) кодировались символы вне текущей кодовой страницы.
fn unescape_unicode_literals(input: &str) -> String {
    if !input.contains("\\U+") && !input.contains("\\u+") {
        return input.to_string();
    }

    let bytes = input.as_bytes();
    let mut result = String::with_capacity(input.len());
    let mut i = 0;
    while i < bytes.len() {
        let is_marker = i + 3 <= bytes.len()
            && bytes[i] == b'\\'
            && (bytes[i + 1] == b'U' || bytes[i + 1] == b'u')
            && bytes[i + 2] == b'+';
        if is_marker && i + 7 <= bytes.len() {
            let hex = &input[i + 3..i + 7];
            if let Ok(code) = u32::from_str_radix(hex, 16) {
                if let Some(ch) = char::from_u32(code) {
                    result.push(ch);
                    i += 7;
                    continue;
                }
            }
        }

        let ch = input[i..].chars().next().unwrap();
        result.push(ch);
        i += ch.len_utf8();
    }
    result
}

// Оформление текста, заданное управляющими кодами DXF (%%u, %%o, %%k и т.п.).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct TextStyle {
    pub underline: bool,
    pub overline: bool,
    pub strikethrough: bool,
}

// Разбирает управляющие коды DXF-текста (%%u — подчёркивание, %%o — надчёркивание,
// %%k — зачёркивание, %%d/%%p/%%c — специальные символы °, ±, ⌀, %%NNN — код символа)
// и возвращает очищенный текст вместе с итоговым стилем оформления.
fn process_special_text_codes(input: &str) -> (String, TextStyle) {
    if !input.contains("%%") {
        return (input.to_string(), TextStyle::default());
    }

    let chars: Vec<char> = input.chars().collect();
    let mut result = String::with_capacity(input.len());
    let mut style = TextStyle::default();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '%' && i + 2 < chars.len() && chars[i + 1] == '%' {
            let c = chars[i + 2];
            let lower = c.to_ascii_lowercase();
            match lower {
                'u' => {
                    style.underline = !style.underline;
                    i += 3;
                    continue;
                }
                'o' => {
                    style.overline = !style.overline;
                    i += 3;
                    continue;
                }
                'k' => {
                    style.strikethrough = !style.strikethrough;
                    i += 3;
                    continue;
                }
                'd' => {
                    result.push('°');
                    i += 3;
                    continue;
                }
                'p' => {
                    result.push('±');
                    i += 3;
                    continue;
                }
                'c' => {
                    result.push('⌀');
                    i += 3;
                    continue;
                }
                '%' => {
                    result.push('%');
                    i += 3;
                    continue;
                }
                _ if c.is_ascii_digit() => {
                    if i + 4 < chars.len()
                        && chars[i + 3].is_ascii_digit()
                        && chars[i + 4].is_ascii_digit()
                    {
                        let code: String = chars[i + 2..=i + 4].iter().collect();
                        if let Ok(n) = code.parse::<u32>() {
                            if let Some(ch) = char::from_u32(n) {
                                result.push(ch);
                                i += 5;
                                continue;
                            }
                        }
                    }

                    result.push(chars[i]);
                    i += 1;
                    continue;
                }
                _ => {
                    result.push(chars[i]);
                    i += 1;
                    continue;
                }
            }
        }
        result.push(chars[i]);
        i += 1;
    }
    (result, style)
}

// Реэкспорт списка кодировок для выбора в диалоге (см. encoding.rs).
pub const ENCODING_CHOICES: &[(&str, &'static encoding_rs::Encoding, &str)] =
    crate::encoding::SELECTABLE_ENCODINGS;

// Результат загрузки DXF-файла.
pub enum DxfLoadOutcome {
    // Файл успешно загружен и геометрия готова к отображению.
    Loaded(Geometry),
    // Кодировку текста не удалось определить автоматически — нужно спросить пользователя.
    NeedsEncoding { guess_index: Option<usize> },
}

// Итоговая геометрия чертежа, готовая к отрисовке и сравнению.
pub struct Geometry {
    pub segments: Vec<(Pos2, Pos2)>,
    pub circles: Vec<(Pos2, f32)>,
    pub points: Vec<Pos2>,
    pub texts: Vec<(Pos2, f32, String, TextStyle)>,
    pub bounds: egui::Rect,
    pub entity_count: usize,
    pub encoding_label: String,
}

// Промежуточный накопитель геометрии во время обхода сущностей DXF.
struct Collected {
    segments: Vec<(Pos2, Pos2)>,
    circles: Vec<(Pos2, f32)>,
    points: Vec<Pos2>,
    texts: Vec<(Pos2, f32, String, TextStyle)>,
    min: Pos2,
    max: Pos2,
    count: usize,
}

impl Collected {
    // Расширяет охватывающий прямоугольник (min/max), чтобы включить точку p.
    fn touch(&mut self, p: Pos2) {
        if p.x < self.min.x {
            self.min.x = p.x;
        }
        if p.y < self.min.y {
            self.min.y = p.y;
        }
        if p.x > self.max.x {
            self.max.x = p.x;
        }
        if p.y > self.max.y {
            self.max.y = p.y;
        }
    }
}

// Читает DXF-файл с диска, определяет (или использует заданную) кодировку текста,
// разбирает содержимое через крейт `dxf` и собирает геометрию для отрисовки.
pub fn load_dxf(
    path: &Path,
    forced_encoding: Option<&'static encoding_rs::Encoding>,
) -> Result<DxfLoadOutcome, String> {
    let raw_bytes = std::fs::read(path).map_err(|e| format!("{}", e))?;

    let (utf8_text, encoding_label) = if let Some(enc) = forced_encoding {
        let text = crate::encoding::decode_with(&raw_bytes, enc);
        (text, format!("{} (manual)", enc.name()))
    } else {
        match crate::encoding::detect_and_decode(&raw_bytes) {
            crate::encoding::DetectOutcome::Detected { text, info } => {
                let label = if info.had_bom {
                    format!("{} [{}, BOM]", info.label, info.encoding.name())
                } else {
                    info.label
                };
                (text, label)
            }
            crate::encoding::DetectOutcome::NeedsUserChoice { guess_index } => {
                return Ok(DxfLoadOutcome::NeedsEncoding { guess_index });
            }
        }
    };

    let drawing = dxf::Drawing::load_with_encoding(
        &mut Cursor::new(utf8_text.into_bytes()),
        encoding_rs::UTF_8,
    )
    .map_err(|e| format!("{}", e))?;

    let mut out = Collected {
        segments: Vec::new(),
        circles: Vec::new(),
        points: Vec::new(),
        texts: Vec::new(),
        min: Pos2::new(f32::MAX, f32::MAX),
        max: Pos2::new(f32::MIN, f32::MIN),
        count: 0,
    };

    // Обход начинается без трансформации координат (единичный масштаб, без вставок).
    let identity = |x: f64, y: f64| -> (f64, f64) { (x, y) };
    collect_entities(drawing.entities(), &drawing, &identity, 1.0, 0, &mut out);

    // Если геометрия не найдена (или координаты не определены), используем
    // условную рамку по умолчанию, чтобы избежать пустого/бесконечного вида.
    if !out.min.x.is_finite()
        || !out.max.x.is_finite()
        || (out.segments.is_empty()
            && out.circles.is_empty()
            && out.points.is_empty()
            && out.texts.is_empty())
    {
        out.min = Pos2::new(-10.0, -10.0);
        out.max = Pos2::new(10.0, 10.0);
    }

    Ok(DxfLoadOutcome::Loaded(Geometry {
        segments: out.segments,
        circles: out.circles,
        points: out.points,
        texts: out.texts,
        bounds: egui::Rect::from_min_max(out.min, out.max),
        entity_count: out.count,
        encoding_label,
    }))
}

// Рекурсивно обходит сущности DXF (включая вложенные блоки через Insert)
// и добавляет их в `out` в виде отрезков/окружностей/точек/надписей.
// `xform` переводит координаты сущности в мировые координаты чертежа,
// `scale_len` — накопленный коэффициент масштаба (для радиусов, высоты текста и т.п.).
fn collect_entities<'a>(
    entities: impl Iterator<Item = &'a dxf::entities::Entity>,
    drawing: &dxf::Drawing,
    xform: &dyn Fn(f64, f64) -> (f64, f64),
    scale_len: f64,
    depth: u32,
    out: &mut Collected,
) {
    // Защита от зацикливания при рекурсивных/некорректных вставках блоков.
    if depth > 8 {
        return;
    }

    let to_pos = |x: f64, y: f64| -> Pos2 {
        let (wx, wy) = xform(x, y);
        Pos2::new(wx as f32, wy as f32)
    };

    for entity in entities {
        use dxf::entities::EntityType::*;
        match &entity.specific {
            Line(line) => {
                let a = to_pos(line.p1.x, line.p1.y);
                let b = to_pos(line.p2.x, line.p2.y);
                out.touch(a);
                out.touch(b);
                out.segments.push((a, b));
                out.count += 1;
            }
            Circle(c) => {
                let center = to_pos(c.center.x, c.center.y);
                let r = (c.radius * scale_len) as f32;
                out.touch(Pos2::new(center.x - r, center.y - r));
                out.touch(Pos2::new(center.x + r, center.y + r));
                out.circles.push((center, r));
                out.count += 1;
            }
            Arc(arc) => {
                // Дуга аппроксимируется ломаной из фиксированного числа отрезков.
                let steps = 48;
                let start = arc.start_angle.to_radians();
                let mut end = arc.end_angle.to_radians();
                if end < start {
                    end += std::f64::consts::TAU;
                }
                let mut prev: Option<Pos2> = None;
                for i in 0..=steps {
                    let frac = i as f64 / steps as f64;
                    let theta = start + (end - start) * frac;
                    let lx = arc.center.x + arc.radius * theta.cos();
                    let ly = arc.center.y + arc.radius * theta.sin();
                    let p = to_pos(lx, ly);
                    out.touch(p);
                    if let Some(prev_p) = prev {
                        out.segments.push((prev_p, p));
                    }
                    prev = Some(p);
                }
                out.count += 1;
            }
            Ellipse(e) => {
                // Эллипс (дуга эллипса) аппроксимируется ломаной в его собственной
                // системе координат, затем поворачивается на угол большой оси.
                let major_len = (e.major_axis.x.powi(2) + e.major_axis.y.powi(2)).sqrt();
                let axis_angle = e.major_axis.y.atan2(e.major_axis.x);
                let start = e.start_parameter;
                let mut end = e.end_parameter;
                if end <= start {
                    end += std::f64::consts::TAU;
                }
                let steps = 64;
                let mut prev: Option<Pos2> = None;
                for i in 0..=steps {
                    let frac = i as f64 / steps as f64;
                    let t = start + (end - start) * frac;
                    let lx = major_len * t.cos();
                    let ly = major_len * e.minor_axis_ratio * t.sin();
                    let (s, c) = axis_angle.sin_cos();
                    let rx = lx * c - ly * s;
                    let ry = lx * s + ly * c;
                    let p = to_pos(e.center.x + rx, e.center.y + ry);
                    out.touch(p);
                    if let Some(prev_p) = prev {
                        out.segments.push((prev_p, p));
                    }
                    prev = Some(p);
                }
                out.count += 1;
            }
            LwPolyline(poly) => {
                // Лёгкая полилиния: соединяем вершины последовательно,
                // замыкая контур, если полилиния помечена как закрытая.
                let pts: Vec<Pos2> = poly.vertices.iter().map(|v| to_pos(v.x, v.y)).collect();
                for p in &pts {
                    out.touch(*p);
                }
                for i in 0..pts.len().saturating_sub(1) {
                    out.segments.push((pts[i], pts[i + 1]));
                }
                if pts.len() > 2 && poly.get_is_closed() {
                    out.segments.push((pts[pts.len() - 1], pts[0]));
                }
                out.count += 1;
            }
            Polyline(poly) => {
                let pts: Vec<Pos2> = poly
                    .vertices()
                    .map(|v| to_pos(v.location.x, v.location.y))
                    .collect();
                for p in &pts {
                    out.touch(*p);
                }
                for i in 0..pts.len().saturating_sub(1) {
                    out.segments.push((pts[i], pts[i + 1]));
                }
                if pts.len() > 2 && poly.get_is_closed() {
                    out.segments.push((pts[pts.len() - 1], pts[0]));
                }
                out.count += 1;
            }
            ModelPoint(pt) => {
                let p = to_pos(pt.location.x, pt.location.y);
                out.touch(p);
                out.points.push(p);
                out.count += 1;
            }
            Solid(s) => {
                // Четырёхугольник SOLID рисуется как замкнутый контур из 4 отрезков
                // (третья и четвёртая вершины в DXF идут по диагонали, не по периметру).
                let a = to_pos(s.first_corner.x, s.first_corner.y);
                let b = to_pos(s.second_corner.x, s.second_corner.y);
                let c = to_pos(s.third_corner.x, s.third_corner.y);
                let d = to_pos(s.fourth_corner.x, s.fourth_corner.y);
                for p in [a, b, c, d] {
                    out.touch(p);
                }

                out.segments.push((a, b));
                out.segments.push((b, d));
                out.segments.push((d, c));
                out.segments.push((c, a));
                out.count += 1;
            }
            Trace(tr) => {
                let a = to_pos(tr.first_corner.x, tr.first_corner.y);
                let b = to_pos(tr.second_corner.x, tr.second_corner.y);
                let c = to_pos(tr.third_corner.x, tr.third_corner.y);
                let d = to_pos(tr.fourth_corner.x, tr.fourth_corner.y);
                for p in [a, b, c, d] {
                    out.touch(p);
                }
                out.segments.push((a, b));
                out.segments.push((b, d));
                out.segments.push((d, c));
                out.segments.push((c, a));
                out.count += 1;
            }
            Spline(sp) => {
                // Сплайн аппроксимируется ломаной по опорным точкам: используются
                // fit-точки, если они заданы, иначе — контрольные точки.
                let src: Vec<&dxf::Point> = if !sp.fit_points.is_empty() {
                    sp.fit_points.iter().collect()
                } else {
                    sp.control_points.iter().collect()
                };
                let pts: Vec<Pos2> = src.iter().map(|v| to_pos(v.x, v.y)).collect();
                for p in &pts {
                    out.touch(*p);
                }
                for i in 0..pts.len().saturating_sub(1) {
                    out.segments.push((pts[i], pts[i + 1]));
                }
                out.count += 1;
            }
            Text(txt) => {
                let p = to_pos(txt.location.x, txt.location.y);
                out.touch(p);
                let h = (txt.text_height * scale_len) as f32;
                let unescaped = unescape_unicode_literals(&txt.value);
                let (clean, style) = process_special_text_codes(&unescaped);
                out.texts.push((p, h.max(0.1), clean, style));
                out.count += 1;
            }
            Ray(ray) => {
                // Луч рисуется как очень длинный отрезок в заданном направлении.
                let a = to_pos(ray.start_point.x, ray.start_point.y);
                let far = 1.0e6;
                let b = to_pos(
                    ray.start_point.x + ray.unit_direction_vector.x * far,
                    ray.start_point.y + ray.unit_direction_vector.y * far,
                );
                out.touch(a);
                out.segments.push((a, b));
                out.count += 1;
            }
            XLine(xl) => {
                // Бесконечная прямая рисуется как очень длинный отрезок в обе стороны.
                let far = 1.0e6;
                let a = to_pos(
                    xl.first_point.x - xl.unit_direction_vector.x * far,
                    xl.first_point.y - xl.unit_direction_vector.y * far,
                );
                let b = to_pos(
                    xl.first_point.x + xl.unit_direction_vector.x * far,
                    xl.first_point.y + xl.unit_direction_vector.y * far,
                );
                out.segments.push((a, b));
                out.count += 1;
            }
            Insert(ins) => {
                // Вставка блока: строим новую трансформацию (смещение, масштаб,
                // поворот) и рекурсивно обходим сущности блока в этой системе координат.
                if let Some(block) = drawing.blocks().find(|b| b.name == ins.name) {
                    let bx = block.base_point.x;
                    let by = block.base_point.y;
                    let sx = ins.x_scale_factor;
                    let sy = ins.y_scale_factor;
                    let rot = ins.rotation.to_radians();
                    let tx = ins.location.x;
                    let ty = ins.location.y;

                    let new_xform = move |x: f64, y: f64| -> (f64, f64) {
                        let lx = (x - bx) * sx;
                        let ly = (y - by) * sy;
                        let (s, c) = rot.sin_cos();
                        let rx = lx * c - ly * s;
                        let ry = lx * s + ly * c;
                        xform(rx + tx, ry + ty)
                    };
                    let new_scale_len = scale_len * ((sx.abs() + sy.abs()) / 2.0);
                    collect_entities(
                        block.entities.iter(),
                        drawing,
                        &new_xform,
                        new_scale_len,
                        depth + 1,
                        out,
                    );
                }
            }
            _ => {}
        }
    }
}
