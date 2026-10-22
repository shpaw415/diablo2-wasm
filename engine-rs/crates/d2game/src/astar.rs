//! A* pathfinding on the sub-tile walkability grid — 8 directions with
//! diagonal costs (10 orthogonal / 14 diagonal, i.e. ~√2).
//!
//! This lives in d2game (renderer-adjacent) so the MVP can ship before
//! `d2rules` implements it; the contract lets d2rules call in for paths.

use std::collections::BinaryHeap;

const STRAIGHT: i32 = 10;
const DIAGONAL: i32 = 14;

#[derive(PartialEq)]
struct Node {
    f: i32,
    g: i32,
    x: usize,
    y: usize,
}

impl Eq for Node {}

impl Ord for Node {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other.f.cmp(&self.f).then_with(|| other.g.cmp(&self.g)) // max-heap → lowest f first
    }
}

impl PartialOrd for Node {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[inline]
fn heuristic(x: usize, y: usize, gx: usize, gy: usize) -> i32 {
    let dx = x.abs_diff(gx);
    let dy = y.abs_diff(gy);
    STRAIGHT * dx.abs_diff(dy) as i32 + DIAGONAL * dx.min(dy) as i32
}

/// Shortest path from `start` to `goal` (both `(x, y)` grid columns/rows),
/// excluding `start`, including `goal`. Diagonal steps require both
/// adjacent orthogonals to be walkable (no corner cutting). `None` when
/// either endpoint is blocked or no path exists.
pub fn astar(
    walk: &[bool],
    w: usize,
    h: usize,
    start: (usize, usize),
    goal: (usize, usize),
) -> Option<Vec<(usize, usize)>> {
    let (sx, sy) = start;
    let (gx, gy) = goal;
    if sx >= w || sy >= h || gx >= w || gy >= h {
        return None;
    }
    if !walk[sy * w + sx] || !walk[gy * w + gx] {
        return None;
    }
    if start == goal {
        return Some(Vec::new());
    }

    let mut open = BinaryHeap::new();
    let mut g_score = vec![i32::MAX; w * h];
    let mut came_from: Vec<usize> = vec![usize::MAX; w * h];
    let s = sy * w + sx;
    g_score[s] = 0;
    open.push(Node { f: heuristic(sx, sy, gx, gy), g: 0, x: sx, y: sy });

    const DIRS: [(isize, isize, i32); 8] = [
        (1, 0, STRAIGHT),
        (-1, 0, STRAIGHT),
        (0, 1, STRAIGHT),
        (0, -1, STRAIGHT),
        (1, 1, DIAGONAL),
        (1, -1, DIAGONAL),
        (-1, 1, DIAGONAL),
        (-1, -1, DIAGONAL),
    ];

    while let Some(Node { f: _, g, x, y }) = open.pop() {
        let cur = y * w + x;
        if g > g_score[cur] {
            continue; // stale heap entry
        }
        if (x, y) == (gx, gy) {
            let mut path = Vec::new();
            let mut at = cur;
            while at != s {
                path.push((at % w, at / w));
                at = came_from[at];
            }
            path.reverse();
            return Some(path);
        }
        for (dx, dy, cost) in DIRS {
            let nx = x as isize + dx;
            let ny = y as isize + dy;
            if nx < 0 || ny < 0 || nx >= w as isize || ny >= h as isize {
                continue;
            }
            let (nx, ny) = (nx as usize, ny as usize);
            if !walk[ny * w + nx] {
                continue;
            }
            if dx != 0 && dy != 0 && (!walk[y * w + nx] || !walk[ny * w + x]) {
                continue; // no corner cutting
            }
            let ng = g + cost;
            let ni = ny * w + nx;
            if ng < g_score[ni] {
                g_score[ni] = ng;
                came_from[ni] = cur;
                open.push(Node {
                    f: ng + heuristic(nx, ny, gx, gy),
                    g: ng,
                    x: nx,
                    y: ny,
                });
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid(rows: &[&str]) -> (Vec<bool>, usize, usize) {
        let w = rows[0].len();
        let h = rows.len();
        let walk: Vec<bool> = rows.iter().flat_map(|r| r.bytes().map(|b| b == b'.')).collect();
        (walk, w, h)
    }

    #[test]
    fn straight_line() {
        let (walk, w, h) = grid(&["...", "...", "..."]);
        let path = astar(&walk, w, h, (0, 0), (2, 0)).unwrap();
        assert_eq!(path, vec![(1, 0), (2, 0)]);
    }

    #[test]
    fn diagonal_is_shorter_than_two_straights() {
        let (walk, w, h) = grid(&["...", "...", "..."]);
        let path = astar(&walk, w, h, (0, 0), (2, 2)).unwrap();
        assert_eq!(path, vec![(1, 1), (2, 2)]); // diagonal hops, not L-shape
    }

    #[test]
    fn around_obstacle() {
        let (walk, w, h) = grid(&["...", ".#.", "..."]);
        let path = astar(&walk, w, h, (0, 1), (2, 1)).unwrap();
        assert!(path.contains(&(1, 0)) || path.contains(&(1, 2)));
        assert_eq!(*path.last().unwrap(), (2, 1));
    }

    #[test]
    fn no_corner_cutting() {
        let (walk, w, h) = grid(&["..", "##", ".."]);
        // Diagonal from (0,0) to (1,1) is forbidden: both orthogonals blocked.
        let path = astar(&walk, w, h, (0, 0), (1, 2));
        assert!(path.is_none()); // sealed by the wall row
    }

    #[test]
    fn blocked_endpoints_rejected() {
        let (walk, w, h) = grid(&[".#.", "..."]);
        assert!(astar(&walk, w, h, (1, 0), (2, 1)).is_none());
        assert!(astar(&walk, w, h, (0, 0), (9, 9)).is_none());
    }
}
