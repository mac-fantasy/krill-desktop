//! Window policy in physical pixels, independent of Win32 and GPUI rendering.
pub const BALL_SIZE: f64 = 88.;
pub const BAR_WIDTH: f64 = 16.;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub width: i32,
    pub height: i32,
}
impl Rect {
    pub fn right(self) -> i32 {
        self.left + self.width
    }
    pub fn bottom(self) -> i32 {
        self.top + self.height
    }
    pub fn clamp(self, area: Self, margin: i32) -> Self {
        Self {
            left: self.left.clamp(
                area.left + margin,
                (area.right() - self.width - margin).max(area.left + margin),
            ),
            top: self.top.clamp(
                area.top + margin,
                (area.bottom() - self.height - margin).max(area.top + margin),
            ),
            ..self
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}
pub fn dock_side(rect: Rect, area: Rect, scale: f64, force: bool) -> Option<Side> {
    let left = (rect.left - area.left).abs();
    let right = (rect.right() - area.right()).abs();
    if !force && left.min(right) as f64 > 12. * scale {
        None
    } else {
        Some(if left <= right {
            Side::Left
        } else {
            Side::Right
        })
    }
}
pub fn docked(rect: Rect, area: Rect, scale: f64, side: Side, collapsed: bool) -> Rect {
    let width = ((if collapsed { BAR_WIDTH } else { BALL_SIZE }) * scale).round() as i32;
    let height = (BALL_SIZE * scale).round() as i32;
    Rect {
        left: match side {
            Side::Left => area.left + 2,
            Side::Right => area.right() - width - 2,
        },
        top: rect.top,
        width,
        height,
    }
    .clamp(area, 2)
}
pub fn expanded_anchor(rect: Rect, scale: f64, side: Option<Side>, collapsed: bool) -> (i32, i32) {
    (
        if collapsed && side == Some(Side::Right) {
            rect.right() - (BALL_SIZE * scale).round() as i32
        } else {
            rect.left
        },
        rect.top,
    )
}
pub fn panel(anchor: Rect, area: Rect, scale: f64) -> Rect {
    let width = ((520. * scale).round() as i32).min((area.width - 16).max(1));
    let height = ((800. * scale).round() as i32).min((area.height - 16).max(1));
    let gap = (8. * scale).round() as i32;
    let left = if anchor.left - width - gap < area.left {
        anchor.right() + gap
    } else {
        anchor.left - width - gap
    };
    Rect {
        left,
        top: anchor.top - (140. * scale).round() as i32,
        width,
        height,
    }
    .clamp(area, 8)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn docking_preserves_expanded_anchor_at_both_edges_and_dpi_scales() {
        for scale in [1., 1.25, 1.5, 2.] {
            for side in [Side::Left, Side::Right] {
                let area = Rect {
                    left: -2560,
                    top: -200,
                    width: 2560,
                    height: 1440,
                };
                let original = Rect {
                    left: -2000,
                    top: 900,
                    width: 88,
                    height: 88,
                };
                let expanded = docked(original, area, scale, side, false);
                let collapsed = docked(expanded, area, scale, side, true);
                assert_eq!(collapsed.width, (16. * scale).round() as i32);
                assert_eq!(
                    expanded_anchor(collapsed, scale, Some(side), true),
                    (expanded.left, expanded.top)
                );
                assert_eq!(docked(collapsed, area, scale, side, false), expanded);
                assert_eq!(dock_side(expanded, area, scale, false), Some(side));
            }
        }
    }
    #[test]
    fn panel_placement_handles_negative_monitors_and_small_work_areas() {
        for area in [
            Rect {
                left: -1920,
                top: 0,
                width: 1920,
                height: 1040,
            },
            Rect {
                left: 0,
                top: -900,
                width: 800,
                height: 600,
            },
        ] {
            for anchor in [
                Rect {
                    left: area.left + 2,
                    top: area.top + 10,
                    width: 88,
                    height: 88,
                },
                Rect {
                    left: area.right() - 90,
                    top: area.bottom() - 100,
                    width: 88,
                    height: 88,
                },
            ] {
                let p = panel(anchor, area, 2.);
                assert!(
                    p.left >= area.left
                        && p.top >= area.top
                        && p.right() <= area.right()
                        && p.bottom() <= area.bottom()
                );
            }
        }
    }
    #[test]
    fn unforced_docking_does_not_snap_middle_of_screen() {
        let area = Rect {
            left: 0,
            top: 0,
            width: 1920,
            height: 1040,
        };
        let rect = Rect {
            left: 800,
            top: 200,
            width: 88,
            height: 88,
        };
        assert_eq!(dock_side(rect, area, 1., false), None);
        assert_eq!(dock_side(rect, area, 1., true), Some(Side::Left));
    }
}
