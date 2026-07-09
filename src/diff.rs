// Вычисление разницы между двумя чертежами: сравниваются отрезки, окружности,
// точки и надписи после приведения обоих файлов к общему масштабу.
use crate::geometry::{Geometry, TextStyle};
use egui::{Pos2, Vec2};

// Геометрия, присутствующая только в одном из файлов (после сравнения).
pub struct DiffResult {
    pub only_a: Vec<(Pos2, Pos2)>,
    pub only_b: Vec<(Pos2, Pos2)>,
    pub only_a_circles: Vec<(Pos2, f32)>,
    pub only_b_circles: Vec<(Pos2, f32)>,
    pub only_a_points: Vec<Pos2>,
    pub only_b_points: Vec<Pos2>,
    pub only_a_texts: Vec<(Pos2, f32, String, TextStyle)>,
    pub only_b_texts: Vec<(Pos2, f32, String, TextStyle)>,
}

// Сравнивает геометрию двух файлов и возвращает несовпадающие участки.
// Оба чертежа сначала масштабируются относительно своего центра (scale_a/scale_b),
// чтобы совпадающие по форме, но по-разному отмасштабированные объекты не считались различием.
pub fn compute_diff(a: &Geometry, scale_a: f32, b: &Geometry, scale_b: f32) -> DiffResult {
    let center_a = a.bounds.center();
    let center_b = b.bounds.center();

    let seg_a: Vec<(Pos2, Pos2)> = a
        .segments
        .iter()
        .map(|(p, q)| {
            (
                scale_pt(*p, center_a, scale_a),
                scale_pt(*q, center_a, scale_a),
            )
        })
        .collect();
    let seg_b: Vec<(Pos2, Pos2)> = b
        .segments
        .iter()
        .map(|(p, q)| {
            (
                scale_pt(*p, center_b, scale_b),
                scale_pt(*q, center_b, scale_b),
            )
        })
        .collect();

    let a_min_s = scale_pt(a.bounds.min, center_a, scale_a);
    let a_max_s = scale_pt(a.bounds.max, center_a, scale_a);
    let b_min_s = scale_pt(b.bounds.min, center_b, scale_b);
    let b_max_s = scale_pt(b.bounds.max, center_b, scale_b);
    let bmin = Pos2::new(a_min_s.x.min(b_min_s.x), a_min_s.y.min(b_min_s.y));
    let bmax = Pos2::new(a_max_s.x.max(b_max_s.x), a_max_s.y.max(b_max_s.y));
    let diag = ((bmax.x - bmin.x).powi(2) + (bmax.y - bmin.y).powi(2))
        .sqrt()
        .max(1e-6);

    // Допуск считается от диагонали общего охватывающего прямоугольника:
    // так порог остаётся адекватным как для чертежа в миллиметрах, так и в метрах.
    let dist_tol = (diag * 5e-6).max(1e-5);

    let only_a = diff_one_side(&seg_a, &seg_b, dist_tol);
    let only_b = diff_one_side(&seg_b, &seg_a, dist_tol);

    let (only_a_circles, only_b_circles) = diff_circles(
        &a.circles, center_a, scale_a, &b.circles, center_b, scale_b, dist_tol,
    );

    let (only_a_points, only_b_points) = diff_points(
        &a.points, center_a, scale_a, &b.points, center_b, scale_b, dist_tol,
    );

    let (only_a_texts, only_b_texts) = diff_texts(
        &a.texts, center_a, scale_a, &b.texts, center_b, scale_b, dist_tol,
    );

    DiffResult {
        only_a,
        only_b,
        only_a_circles,
        only_b_circles,
        only_a_points,
        only_b_points,
        only_a_texts,
        only_b_texts,
    }
}

// Масштабирует точку относительно заданного центра.
fn scale_pt(p: Pos2, center: Pos2, s: f32) -> Pos2 {
    Pos2::new(
        center.x + (p.x - center.x) * s,
        center.y + (p.y - center.y) * s,
    )
}

// Возвращает участки отрезков `mine`, которые не покрываются ни одним
// коллинеарным отрезком из `other` (с учётом допуска dist_tol).
fn diff_one_side(
    mine: &[(Pos2, Pos2)],
    other: &[(Pos2, Pos2)],
    dist_tol: f32,
) -> Vec<(Pos2, Pos2)> {
    let mut result = Vec::new();

    for &(p0, p1) in mine {
        let d = p1 - p0;
        let len = d.length();
        if len < 1e-9 {
            continue;
        }
        let dir = d / len;

        let mut covered: Vec<(f32, f32)> = Vec::new();
        for &(q0, q1) in other {
            if let Some(iv) = collinear_overlap(p0, dir, len, q0, q1, dist_tol) {
                covered.push(iv);
            }
        }

        for (t0, t1) in subtract_intervals(0.0, 1.0, &mut covered) {
            if (t1 - t0) * len < dist_tol {
                continue;
            }
            let a = p0 + dir * (t0 * len);
            let b = p0 + dir * (t1 * len);
            result.push((a, b));
        }
    }

    result
}

// Если отрезок [q0,q1] лежит на той же прямой, что и отрезок,
// заданный точкой p0 и направлением dir (длина len), возвращает диапазон
// параметра t (0..1) вдоль первого отрезка, который покрывается вторым.
// Иначе возвращает None (отрезки не коллинеарны или не перекрываются).
fn collinear_overlap(
    p0: Pos2,
    dir: Vec2,
    len: f32,
    q0: Pos2,
    q1: Pos2,
    dist_tol: f32,
) -> Option<(f32, f32)> {
    let qd = q1 - q0;
    let qlen = qd.length();
    if qlen < 1e-9 {
        return None;
    }
    let qdir = qd / qlen;

    let cross = dir.x * qdir.y - dir.y * qdir.x;
    if cross.abs() > 5e-4 {
        return None;
    }

    let perp = |p: Pos2| -> f32 {
        let v = p - p0;
        (v.x * dir.y - v.y * dir.x).abs()
    };
    if perp(q0) > dist_tol || perp(q1) > dist_tol {
        return None;
    }

    let proj = |p: Pos2| -> f32 {
        let v = p - p0;
        (v.x * dir.x + v.y * dir.y) / len
    };
    let mut t0 = proj(q0);
    let mut t1 = proj(q1);
    if t0 > t1 {
        std::mem::swap(&mut t0, &mut t1);
    }

    if t1 < 0.0 || t0 > 1.0 {
        return None;
    }
    Some((t0.max(0.0), t1.min(1.0)))
}

// Вычитает объединение заданных интервалов из отрезка [start, end]
// и возвращает оставшиеся (непокрытые) части.
fn subtract_intervals(start: f32, end: f32, intervals: &mut [(f32, f32)]) -> Vec<(f32, f32)> {
    if intervals.is_empty() {
        return vec![(start, end)];
    }

    intervals.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    let mut merged: Vec<(f32, f32)> = Vec::new();
    for &(s, e) in intervals.iter() {
        if let Some(last) = merged.last_mut() {
            if s <= last.1 + 1e-7 {
                if e > last.1 {
                    last.1 = e;
                }
                continue;
            }
        }
        merged.push((s, e));
    }

    let mut result = Vec::new();
    let mut cursor = start;
    for (s, e) in merged {
        if s > cursor {
            result.push((cursor, s.min(end)));
        }
        if e > cursor {
            cursor = e;
        }
        if cursor >= end {
            break;
        }
    }
    if cursor < end {
        result.push((cursor, end));
    }
    result
}

// Находит окружности, присутствующие только в одном из файлов
// (совпадение определяется по центру и радиусу с учётом допуска).
fn diff_circles(
    a: &[(Pos2, f32)],
    center_a: Pos2,
    scale_a: f32,
    b: &[(Pos2, f32)],
    center_b: Pos2,
    scale_b: f32,
    dist_tol: f32,
) -> (Vec<(Pos2, f32)>, Vec<(Pos2, f32)>) {
    let a_s: Vec<(Pos2, f32)> = a
        .iter()
        .map(|(c, r)| (scale_pt(*c, center_a, scale_a), r * scale_a))
        .collect();
    let b_s: Vec<(Pos2, f32)> = b
        .iter()
        .map(|(c, r)| (scale_pt(*c, center_b, scale_b), r * scale_b))
        .collect();

    let has_match = |c: Pos2, r: f32, other: &[(Pos2, f32)]| -> bool {
        other.iter().any(|(oc, or_)| {
            let dc = ((c.x - oc.x).powi(2) + (c.y - oc.y).powi(2)).sqrt();
            dc <= dist_tol && (r - or_).abs() <= dist_tol
        })
    };

    let only_a: Vec<(Pos2, f32)> = a_s
        .iter()
        .filter(|(c, r)| !has_match(*c, *r, &b_s))
        .cloned()
        .collect();
    let only_b: Vec<(Pos2, f32)> = b_s
        .iter()
        .filter(|(c, r)| !has_match(*c, *r, &a_s))
        .cloned()
        .collect();
    (only_a, only_b)
}

// Находит точки, присутствующие только в одном из файлов.
fn diff_points(
    a: &[Pos2],
    center_a: Pos2,
    scale_a: f32,
    b: &[Pos2],
    center_b: Pos2,
    scale_b: f32,
    dist_tol: f32,
) -> (Vec<Pos2>, Vec<Pos2>) {
    let a_s: Vec<Pos2> = a.iter().map(|p| scale_pt(*p, center_a, scale_a)).collect();
    let b_s: Vec<Pos2> = b.iter().map(|p| scale_pt(*p, center_b, scale_b)).collect();

    let has_match = |p: Pos2, other: &[Pos2]| -> bool {
        other.iter().any(|q| {
            let dc = ((p.x - q.x).powi(2) + (p.y - q.y).powi(2)).sqrt();
            dc <= dist_tol
        })
    };

    let only_a: Vec<Pos2> = a_s
        .iter()
        .filter(|p| !has_match(**p, &b_s))
        .cloned()
        .collect();
    let only_b: Vec<Pos2> = b_s
        .iter()
        .filter(|p| !has_match(**p, &a_s))
        .cloned()
        .collect();
    (only_a, only_b)
}

// Находит надписи, присутствующие только в одном из файлов.
// Совпадением считается одинаковый текст, стиль и близкая позиция.
fn diff_texts(
    a: &[(Pos2, f32, String, TextStyle)],
    center_a: Pos2,
    scale_a: f32,
    b: &[(Pos2, f32, String, TextStyle)],
    center_b: Pos2,
    scale_b: f32,
    dist_tol: f32,
) -> (
    Vec<(Pos2, f32, String, TextStyle)>,
    Vec<(Pos2, f32, String, TextStyle)>,
) {
    let a_s: Vec<(Pos2, f32, String, TextStyle)> = a
        .iter()
        .map(|(p, h, s, st)| (scale_pt(*p, center_a, scale_a), h * scale_a, s.clone(), *st))
        .collect();
    let b_s: Vec<(Pos2, f32, String, TextStyle)> = b
        .iter()
        .map(|(p, h, s, st)| (scale_pt(*p, center_b, scale_b), h * scale_b, s.clone(), *st))
        .collect();

    let has_match =
        |p: Pos2, txt: &str, style: TextStyle, other: &[(Pos2, f32, String, TextStyle)]| -> bool {
            other.iter().any(|(q, _, s2, st2)| {
                let dc = ((p.x - q.x).powi(2) + (p.y - q.y).powi(2)).sqrt();
                dc <= dist_tol && s2 == txt && *st2 == style
            })
        };

    let only_a: Vec<(Pos2, f32, String, TextStyle)> = a_s
        .iter()
        .filter(|(p, _, s, st)| !has_match(*p, s, *st, &b_s))
        .cloned()
        .collect();
    let only_b: Vec<(Pos2, f32, String, TextStyle)> = b_s
        .iter()
        .filter(|(p, _, s, st)| !has_match(*p, s, *st, &a_s))
        .cloned()
        .collect();
    (only_a, only_b)
}
