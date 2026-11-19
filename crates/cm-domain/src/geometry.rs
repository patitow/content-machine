use serde::{Deserialize, Serialize};

/// Retângulo em pixels da fonte (origem no canto superior esquerdo).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl Rect {
    pub fn right(&self) -> u32 {
        self.x + self.w
    }

    pub fn bottom(&self) -> u32 {
        self.y + self.h
    }

    pub fn area(&self) -> u64 {
        u64::from(self.w) * u64::from(self.h)
    }

    /// Só se tocar na borda não é interseção.
    pub fn intersects(&self, other: &Rect) -> bool {
        self.x < other.right()
            && other.x < self.right()
            && self.y < other.bottom()
            && other.y < self.bottom()
    }

    pub fn iou(&self, other: &Rect) -> f64 {
        let ix = self
            .right()
            .min(other.right())
            .saturating_sub(self.x.max(other.x));
        let iy = self
            .bottom()
            .min(other.bottom())
            .saturating_sub(self.y.max(other.y));
        let inter = u64::from(ix) * u64::from(iy);
        let union = self.area() + other.area() - inter;
        if union == 0 {
            return 0.0;
        }
        inter as f64 / union as f64
    }

    pub fn contains_point(&self, x: f64, y: f64) -> bool {
        x >= f64::from(self.x)
            && x < f64::from(self.right())
            && y >= f64::from(self.y)
            && y < f64::from(self.bottom())
    }

    pub fn scaled(&self, factor: f64) -> Rect {
        let s = |v: u32| (f64::from(v) * factor).round() as u32;
        Rect {
            x: s(self.x),
            y: s(self.y),
            w: s(self.w),
            h: s(self.h),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WebcamSource {
    Override,
    Detected,
    Saved,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WebcamDetection {
    pub rect: Rect,
    pub confidence: f64,
    pub source: WebcamSource,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x: u32, y: u32, w: u32, h: u32) -> Rect {
        Rect { x, y, w, h }
    }

    #[test]
    fn iou_of_identical_rects_is_one() {
        assert_eq!(r(3, 4, 10, 20).iou(&r(3, 4, 10, 20)), 1.0);
    }

    #[test]
    fn iou_of_disjoint_rects_is_zero() {
        assert_eq!(r(0, 0, 10, 10).iou(&r(50, 50, 10, 10)), 0.0);
    }

    #[test]
    fn iou_of_half_overlap() {
        let v = r(0, 0, 10, 10).iou(&r(5, 0, 10, 10));
        assert!((v - 50.0 / 150.0).abs() < 1e-9);
    }

    #[test]
    fn touching_edges_do_not_intersect() {
        assert!(!r(0, 0, 10, 10).intersects(&r(10, 0, 10, 10)));
        assert!(r(0, 0, 10, 10).intersects(&r(9, 0, 10, 10)));
    }

    #[test]
    fn scaled_halves_every_field() {
        assert_eq!(r(10, 20, 100, 50).scaled(0.5), r(5, 10, 50, 25));
    }

    #[test]
    fn iou_with_empty_rect_is_zero_without_dividing_by_zero() {
        assert_eq!(r(0, 0, 0, 0).iou(&r(0, 0, 0, 0)), 0.0);
        assert_eq!(r(5, 5, 0, 0).iou(&r(0, 0, 10, 10)), 0.0);
    }
}
