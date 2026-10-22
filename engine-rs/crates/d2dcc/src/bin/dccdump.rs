//! dccdump — smoke/debug CLI: decode DCC/COF files and print stats.

use d2dcc::{Cof, Dcc};

fn main() {
    for arg in std::env::args().skip(1) {
        let data = std::fs::read(&arg).unwrap();
        if arg.to_lowercase().ends_with(".cof") {
            let cof = Cof::parse(&data).unwrap_or_else(|e| panic!("{arg}: {e}"));
            println!(
                "{arg}: layers={} frames={} dirs={} speed={}",
                cof.number_of_layers, cof.frames_per_direction, cof.number_of_directions, cof.speed
            );
            for (i, l) in cof.layers.iter().enumerate() {
                println!(
                    "  layer {i}: {} shadow={} sel={} tr={} effect={} wc={}",
                    l.layer_type.token(),
                    l.shadow,
                    l.selectable,
                    l.transparent,
                    l.draw_effect,
                    l.weapon_class
                );
            }
            println!("  frame0 order: {:?}", cof.draw_order(0, 0));
        } else {
            let dcc = Dcc::decode(&data).unwrap_or_else(|e| panic!("{arg}: {e}"));
            println!(
                "{arg}: v{} dirs={} frames/dir={}",
                dcc.version, dcc.number_of_directions, dcc.frames_per_direction
            );
            for (di, d) in dcc.directions.iter().enumerate() {
                println!(
                    "  dir {di}: box=({},{}) {}x{} outSizeCoded={}",
                    d.box_left, d.box_top, d.box_width, d.box_height, d.out_size_coded
                );
                let mut nonzero = 0usize;
                let mut total = 0usize;
                for (fi, f) in d.frames.iter().enumerate() {
                    let nz = f.pixel_data.iter().filter(|&&p| p != 0).count();
                    nonzero += nz;
                    total += f.pixel_data.len();
                    if di == 0 {
                        println!(
                            "    frame {fi}: box=({},{}) {}x{} cells={}x{} painted={}",
                            f.box_left,
                            f.box_top,
                            f.box_width,
                            f.box_height,
                            f.horizontal_cell_count,
                            f.vertical_cell_count,
                            nz
                        );
                    }
                }
                if di == 0 {
                    let mut hist = [0usize; 256];
                    for &p in &d.frames[0].pixel_data {
                        hist[p as usize] += 1;
                    }
                    let used: Vec<String> = hist
                        .iter()
                        .enumerate()
                        .filter(|(_, &c)| c > 0)
                        .map(|(i, c)| format!("{i}x{c}"))
                        .collect();
                    println!("    frame0 idx histogram: {}", used.join(" "));
                    let entries: Vec<u8> = d.palette_entries.to_vec();
                    println!(
                        "    palette_entries (first 40): {:?}",
                        &entries[..40]
                    );
                }
                println!("    painted {nonzero}/{total} px across {} frames", d.frames.len());
            }
        }
    }
}

// hex dump helper for evidence: first 64 bytes
fn _unused() {}
