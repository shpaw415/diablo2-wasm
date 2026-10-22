//! A* 8 directions sur la grille de collision (grilles ≤ ~100×100 : plein de marge).

use std::cmp::Reverse;
use std::collections::BinaryHeap;

/// Vue grille : `blocked[y][x] == true` ⇒ tuile bloquée ; hors-bornes = bloqué.
pub(crate) struct Grid<'a> {
    /// Largeur en tuiles.
    pub w: i32,
    /// Hauteur en tuiles.
    pub h: i32,
    /// Grille `[y][x]`.
    pub blocked: &'a [Vec<bool>],
}

impl<'a> Grid<'a> {
    /// Tuile marchable et dans les bornes ?
    pub fn walkable(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.w && y < self.h && !self.blocked[y as usize][x as usize]
    }
}

/// 4 orthogonales d'abord, 4 diagonales ensuite.
pub(crate) const DIRS: [(i32, i32); 8] =
    [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)];

/// A* octile. Renvoie le chemin EXCLUANT le départ, INCLUANT l'arrivée, en tuiles.
/// `Some(vec![])` si départ == arrivée. `None` si arrivée bloquée (appeler
/// [`snap_near`] d'abord) ou injoignable.
pub(crate) fn astar(g: &Grid, start: (i32, i32), goal: (i32, i32)) -> Option<Vec<(i32, i32)>> {
    if !g.walkable(start.0, start.1) || !g.walkable(goal.0, goal.1) {
        return None;
    }
    if start == goal {
        return Some(Vec::new());
    }
    let w = g.w as usize;
    let idx = |x: i32, y: i32| -> usize { y as usize * w + x as usize };
    let heur = |x: i32, y: i32| -> u32 {
        let dx = (x - goal.0).unsigned_abs();
        let dy = (y - goal.1).unsigned_abs();
        10 * dx.max(dy) + 4 * dx.min(dy) // octile ×10 (coût 10 / 14)
    };
    let n = (g.w * g.h) as usize;
    let mut gscore = vec![u32::MAX; n];
    let mut came = vec![usize::MAX; n];
    let s = idx(start.0, start.1);
    let gl = idx(goal.0, goal.1);
    gscore[s] = 0;
    let mut heap = BinaryHeap::new();
    heap.push(Reverse((heur(start.0, start.1), 0u32, s)));
    while let Some(Reverse((_, g_cur, cur))) = heap.pop() {
        if g_cur > gscore[cur] {
            continue; // entrée périmée
        }
        if cur == gl {
            // reconstruction : arrivée -> départ (départ exclu)
            let mut out = Vec::new();
            let mut c = cur;
            while c != s {
                out.push(((c % w) as i32, (c / w) as i32));
                c = came[c];
            }
            out.reverse();
            return Some(out);
        }
        let cx = (cur % w) as i32;
        let cy = (cur / w) as i32;
        for (dx, dy) in DIRS {
            let nx = cx + dx;
            let ny = cy + dy;
            if !g.walkable(nx, ny) {
                continue;
            }
            // pas de coupe de coin : les 2 orthogonales de la diagonale doivent être libres
            if dx != 0 && dy != 0 && (!g.walkable(cx + dx, cy) || !g.walkable(cx, cy + dy)) {
                continue;
            }
            let cost: u32 = if dx != 0 && dy != 0 { 14 } else { 10 };
            let ng = gscore[cur] + cost;
            let ni = idx(nx, ny);
            if ng < gscore[ni] {
                gscore[ni] = ng;
                came[ni] = cur;
                heap.push(Reverse((ng + heur(nx, ny), ng, ni)));
            }
        }
    }
    None
}

/// Si `goal` est bloquée, cherche la tuile marchable la plus proche (anneaux
/// de rayon 1..=max_r). `None` si rien de marchable à portée.
pub(crate) fn snap_near(g: &Grid, goal: (i32, i32), max_r: i32) -> Option<(i32, i32)> {
    if g.walkable(goal.0, goal.1) {
        return Some(goal);
    }
    for r in 1..=max_r {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs() != r && dy.abs() != r {
                    continue; // périmètre de l'anneau seulement
                }
                let (x, y) = (goal.0 + dx, goal.1 + dy);
                if g.walkable(x, y) {
                    return Some((x, y));
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open_grid(w: i32, h: i32) -> Vec<Vec<bool>> {
        vec![vec![false; w as usize]; h as usize]
    }

    #[test]
    fn straight_line() {
        let grid = open_grid(20, 20);
        let g = Grid { w: 20, h: 20, blocked: &grid };
        let p = astar(&g, (2, 2), (7, 2)).unwrap();
        assert_eq!(p.last(), Some(&(7, 2)));
        assert_eq!(p.len(), 5);
    }

    #[test]
    fn detour_around_wall_and_no_corner_cut() {
        let mut grid = open_grid(20, 20);
        for row in grid.iter_mut().take(16) {
            row[10] = true; // mur partiel : passage au sud (y >= 16)
        }
        let g = Grid { w: 20, h: 20, blocked: &grid };
        let p = astar(&g, (5, 5), (15, 5)).unwrap();
        for &(x, y) in &p {
            assert!(!grid[y as usize][x as usize], "chemin traverse le mur en {x},{y}");
        }
        // interdiction de coupe de coin : aucune diagonale ne longe le mur x=10
        for w2 in p.windows(2) {
            let (dx, dy) = (w2[1].0 - w2[0].0, w2[1].1 - w2[0].1);
            if dx.abs() == 1 && dy.abs() == 1 {
                let (x0, y0) = w2[0];
                assert!(g.walkable(x0 + dx, y0) && g.walkable(x0, y0 + dy));
            }
        }
        assert_eq!(*p.last().unwrap(), (15, 5));
    }

    #[test]
    fn unreachable_is_none() {
        let mut grid = open_grid(20, 20);
        for row in grid.iter_mut() {
            row[10] = true; // mur plein : l'est est injoignable
        }
        let g = Grid { w: 20, h: 20, blocked: &grid };
        assert!(astar(&g, (2, 2), (15, 2)).is_none());
    }

    #[test]
    fn snap_finds_walkable_neighbor() {
        let mut grid = open_grid(10, 10);
        grid[5][5] = true; // (5,5) bloquée
        let g = Grid { w: 10, h: 10, blocked: &grid };
        // anneau r=1, scan (dy,dx) : premier périmètre libre = (4,4)
        assert_eq!(snap_near(&g, (5, 5), 3), Some((4, 4)));
        assert_eq!(snap_near(&g, (20, 20), 2), None); // hors grille, rien à portée
    }
}
