use cm_domain::{Rect, WebcamDetection, WebcamSource};
use cm_media::RgbFrame;

use crate::face::Face;

/// Quadros com rosto estável exigidos para aceitar uma detecção.
const MIN_FACE_FRAMES: usize = 5;
/// Gradiente (0..255) que conta como borda 100% firme.
const EDGE_FULL: f32 = 25.0;
/// Pontuação fixa de um lado que coincide com a borda do quadro.
const FRAME_EDGE_SCORE: f32 = 0.5;
/// Abaixo disto, os 4 lados não formam uma moldura: é recorte da pessoa.
const BORDER_MIN_MEAN: f32 = 0.55;
const CUTOUT_MAX_CONFIDENCE: f64 = 0.7;
const MAX_AREA_FRACTION: f64 = 0.25;
/// Pixels que os lados internos entram para dentro do retângulo achado.
const INSET: i32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebcamKind {
    /// Retângulo com moldura nítida (overlay clássico).
    Bordered,
    /// Pessoa recortada sobre o jogo, sem moldura.
    Cutout,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WebcamOutcome {
    Found {
        detection: WebcamDetection,
        kind: WebcamKind,
    },
    NotFound {
        reason: String,
    },
}

fn not_found(reason: &str) -> WebcamOutcome {
    WebcamOutcome::NotFound {
        reason: reason.to_string(),
    }
}

fn median(mut v: Vec<u32>) -> u32 {
    v.sort_unstable();
    v[v.len() / 2]
}

fn center(r: &Rect) -> (f64, f64) {
    (
        f64::from(r.x) + f64::from(r.w) / 2.0,
        f64::from(r.y) + f64::from(r.h) / 2.0,
    )
}

/// Rosto que mais se repete na mesma posição entre os quadros (rostos falsos de
/// jogo mudam de lugar). Devolve a mediana do grupo e quantos quadros o sustentam.
fn seed_face(faces: &[Vec<Face>]) -> Option<(Rect, usize)> {
    let mut best: Option<(usize, f64, Vec<Rect>)> = None;
    for candidate in faces.iter().flatten() {
        let (cx, cy) = center(&candidate.rect);
        let tol = f64::from(candidate.rect.w) * 0.5;
        let mut group = Vec::new();
        let mut conf = 0.0;
        for frame_faces in faces {
            let hit = frame_faces.iter().find(|f| {
                let (x, y) = center(&f.rect);
                (x - cx).abs() <= tol && (y - cy).abs() <= tol
            });
            if let Some(f) = hit {
                group.push(f.rect);
                conf += f.confidence;
            }
        }
        let better = match &best {
            None => true,
            Some((n, c, _)) => group.len() > *n || (group.len() == *n && conf > *c),
        };
        if better {
            best = Some((group.len(), conf, group));
        }
    }
    best.map(|(n, _, group)| {
        let col = |f: fn(&Rect) -> u32| median(group.iter().map(f).collect());
        (
            Rect {
                x: col(|r| r.x),
                y: col(|r| r.y),
                w: col(|r| r.w),
                h: col(|r| r.h),
            },
            n,
        )
    })
}

struct Gray {
    w: usize,
    h: usize,
    data: Vec<f32>,
}

/// Cinza em metade da resolução (média de blocos 2x2).
fn half_gray(f: &RgbFrame) -> Gray {
    let (w, h) = (f.width as usize / 2, f.height as usize / 2);
    let stride = f.width as usize * 3;
    let mut data = Vec::with_capacity(w * h);
    for y in 0..h {
        for x in 0..w {
            let mut s = 0.0;
            for (dy, dx) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
                let o = (2 * y + dy) * stride + (2 * x + dx) * 3;
                s += 0.299 * f32::from(f.data[o])
                    + 0.587 * f32::from(f.data[o + 1])
                    + 0.114 * f32::from(f.data[o + 2]);
            }
            data.push(s / 4.0);
        }
    }
    Gray { w, h, data }
}

/// Percentil 20 ao longo do tempo do gradiente horizontal e vertical: bordas de
/// overlay existem em todos os quadros, bordas de jogo mudam.
fn persistent_edges(grays: &[Gray]) -> (Vec<f32>, Vec<f32>) {
    let (w, h) = (grays[0].w, grays[0].h);
    let mut px = vec![0.0; w * h];
    let mut py = vec![0.0; w * h];
    let k = ((grays.len() - 1) as f32 * 0.2) as usize;
    let mut sx = vec![0.0f32; grays.len()];
    let mut sy = vec![0.0f32; grays.len()];
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let i = y * w + x;
            for (n, g) in grays.iter().enumerate() {
                sx[n] = (g.data[i + 1] - g.data[i]).abs();
                sy[n] = (g.data[i + w] - g.data[i]).abs();
            }
            px[i] = *sx.select_nth_unstable_by(k, f32::total_cmp).1;
            py[i] = *sy.select_nth_unstable_by(k, f32::total_cmp).1;
        }
    }
    (px, py)
}

struct Edges {
    w: i32,
    h: i32,
    px: Vec<f32>,
    py: Vec<f32>,
}

impl Edges {
    /// Linha vertical na coluna `col`, entre as linhas `r0..r1`.
    fn vertical(&self, col: i32, r0: i32, r1: i32) -> f32 {
        let (r0, r1) = (r0.clamp(0, self.h), r1.clamp(0, self.h));
        if r1 <= r0 {
            return 0.0;
        }
        let mut s = 0.0;
        for r in r0..r1 {
            let m = (col - 1..=col + 1)
                .filter(|c| (0..self.w).contains(c))
                .map(|c| self.px[(r * self.w + c) as usize])
                .fold(0.0, f32::max);
            s += m;
        }
        (s / (r1 - r0) as f32 / EDGE_FULL).min(1.0)
    }

    /// Linha horizontal na linha `row`, entre as colunas `c0..c1`.
    fn horizontal(&self, row: i32, c0: i32, c1: i32) -> f32 {
        let (c0, c1) = (c0.clamp(0, self.w), c1.clamp(0, self.w));
        if c1 <= c0 {
            return 0.0;
        }
        let mut s = 0.0;
        for c in c0..c1 {
            let m = (row - 1..=row + 1)
                .filter(|r| (0..self.h).contains(r))
                .map(|r| self.py[(r * self.w + c) as usize])
                .fold(0.0, f32::max);
            s += m;
        }
        (s / (c1 - c0) as f32 / EDGE_FULL).min(1.0)
    }
}

/// Melhor posição de um lado dentro de `lo..=hi`; se o lado puder coincidir com a
/// borda do quadro (`edge`) e nada interno passar de `FRAME_EDGE_SCORE`, vale a borda.
fn pick_side(lo: i32, hi: i32, edge: Option<i32>, score: impl Fn(i32) -> f32) -> (i32, f32) {
    let mut best = (lo, f32::MIN);
    for p in lo..=hi {
        let s = score(p);
        if s > best.1 {
            best = (p, s);
        }
    }
    match edge {
        Some(e) if best.1 < FRAME_EDGE_SCORE => (e, FRAME_EDGE_SCORE),
        // O gradiente à frente marca o último pixel antes do degrau: a borda é o próximo.
        _ => (best.0 + 1, best.1.max(0.0)),
    }
}

struct Border {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
    mean_score: f32,
}

fn find_border(e: &Edges, face: (i32, i32, i32, i32)) -> Border {
    let (fx, fy, fw, fh) = face;
    let (w, h) = (e.w, e.h);
    let mut span_rows = (fy - fh / 4, fy + fh);
    let mut span_cols = (fx - fw / 2, fx + fw + fw / 2);
    let mut result = Border {
        left: 0,
        top: 0,
        right: w,
        bottom: h,
        mean_score: 0.0,
    };
    for _ in 0..2 {
        let l_lo = fx - 3 * fw;
        let (left, sl) = pick_side(l_lo.max(0), fx - fw / 6, (l_lo <= 0).then_some(0), |c| {
            e.vertical(c, span_rows.0, span_rows.1)
        });
        let r_hi = fx + fw + 3 * fw;
        let (right, sr) = pick_side(
            fx + fw + fw / 6,
            r_hi.min(w - 2),
            (r_hi >= w).then_some(w),
            |c| e.vertical(c, span_rows.0, span_rows.1),
        );
        let t_lo = fy - fh * 3 / 2;
        let (top, st) = pick_side(t_lo.max(0), fy - fh / 10, (t_lo <= 0).then_some(0), |r| {
            e.horizontal(r, span_cols.0, span_cols.1)
        });
        let b_hi = fy + fh + 2 * fh;
        let (bottom, sb) = pick_side(
            fy + fh + fh / 5,
            b_hi.min(h - 2),
            (b_hi >= h).then_some(h),
            |r| e.horizontal(r, span_cols.0, span_cols.1),
        );
        result = Border {
            left,
            top,
            right,
            bottom,
            mean_score: (sl + sr + st + sb) / 4.0,
        };
        let (rw, rh) = (right - left, bottom - top);
        span_rows = (top + rh / 8, bottom - rh / 8);
        span_cols = (left + rw / 8, right - rw / 8);
    }
    result
}

/// Retângulo anatômico para pessoa recortada: ombros ≈ 3 larguras de rosto, do topo
/// da cabeça até o tronco (ou a borda inferior do quadro).
fn anatomical(face: &Rect, frame_w: u32, frame_h: u32) -> Rect {
    let (fw, fh) = (f64::from(face.w), f64::from(face.h));
    let cx = center(face).0 - 0.1 * fw;
    let fy = f64::from(face.y);
    let left = (cx - 1.475 * fw).max(0.0);
    let right = (cx + 1.475 * fw).min(f64::from(frame_w));
    let top = (fy - 0.33 * fh).max(0.0);
    let bottom = (fy + 3.2 * fh).min(f64::from(frame_h));
    Rect {
        x: left as u32,
        y: top as u32,
        w: (right - left) as u32,
        h: (bottom - top) as u32,
    }
}

fn plausible(rect: &Rect, face: &Rect, frame_w: u32, frame_h: u32) -> bool {
    let aspect = f64::from(rect.w) / f64::from(rect.h.max(1));
    let face_ratio = f64::from(face.h) / f64::from(rect.h.max(1));
    let area = rect.area() as f64 / (f64::from(frame_w) * f64::from(frame_h));
    (1.0..=2.0).contains(&aspect) && (0.25..=0.7).contains(&face_ratio) && area <= MAX_AREA_FRACTION
}

pub fn detect_webcam(frames: &[RgbFrame], faces: &[Vec<Face>]) -> WebcamOutcome {
    let Some(first) = frames.first() else {
        return not_found("nenhum quadro amostrado");
    };
    let (frame_w, frame_h) = (first.width, first.height);
    let Some((seed, count)) = seed_face(faces) else {
        return not_found("nenhum rosto detectado");
    };
    if count < MIN_FACE_FRAMES {
        return not_found("rosto em poucos quadros");
    }
    if f64::from(seed.h) > 0.45 * f64::from(frame_h)
        || f64::from(seed.w) > 0.45 * f64::from(frame_w)
    {
        return not_found("rosto grande demais para ser um overlay de webcam");
    }
    let persistence = (count as f64 / frames.len() as f64).min(1.0);

    let grays: Vec<Gray> = frames.iter().map(half_gray).collect();
    let (px, py) = persistent_edges(&grays);
    let edges = Edges {
        w: grays[0].w as i32,
        h: grays[0].h as i32,
        px,
        py,
    };
    let half = |v: u32| (v / 2) as i32;
    let mut border = find_border(
        &edges,
        (half(seed.x), half(seed.y), half(seed.w), half(seed.h)),
    );
    // Overlay nunca fica a poucos pixels da borda do quadro: uma "borda" ali é conteúdo
    // da própria webcam (suporte de microfone, cortina), e vale a borda do quadro.
    let (snap_x, snap_y) = (
        (edges.w as f32 * 0.02) as i32,
        (edges.h as f32 * 0.02) as i32,
    );
    if border.left <= snap_x {
        border.left = 0;
    }
    if edges.w - border.right <= snap_x {
        border.right = edges.w;
    }
    if border.top <= snap_y {
        border.top = 0;
    }
    if edges.h - border.bottom <= snap_y {
        border.bottom = edges.h;
    }

    // Lados internos entram 2 px para dentro: a borda achada pode errar 1 px em
    // metade da resolução e o corte nunca pode levar pixel de jogo para a faixa da webcam.
    let inset =
        |pos: i32, is_edge: bool, sign: i32| pos * 2 + if is_edge { 0 } else { sign * INSET };
    let left = inset(border.left, border.left == 0, 1);
    let top = inset(border.top, border.top == 0, 1);
    let right = inset(border.right, border.right == edges.w, -1);
    let bottom = inset(border.bottom, border.bottom == edges.h, -1);
    let rect = Rect {
        x: left.max(0) as u32,
        y: top.max(0) as u32,
        w: (right - left).max(0) as u32,
        h: (bottom - top).max(0) as u32,
    };
    let source = WebcamSource::Detected;
    if border.mean_score >= BORDER_MIN_MEAN
        && border.right > border.left
        && border.bottom > border.top
        && plausible(&rect, &seed, frame_w, frame_h)
    {
        return WebcamOutcome::Found {
            detection: WebcamDetection {
                rect,
                confidence: (f64::from(border.mean_score) * persistence).min(1.0),
                source,
            },
            kind: WebcamKind::Bordered,
        };
    }

    let rect = anatomical(&seed, frame_w, frame_h);
    if rect.area() as f64 > MAX_AREA_FRACTION * f64::from(frame_w) * f64::from(frame_h) {
        return not_found("retângulo da pessoa grande demais para ser um overlay");
    }
    WebcamOutcome::Found {
        detection: WebcamDetection {
            rect,
            confidence: CUTOUT_MAX_CONFIDENCE * persistence,
            source,
        },
        kind: WebcamKind::Cutout,
    }
}
