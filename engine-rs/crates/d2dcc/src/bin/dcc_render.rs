//! dcc_render — render decoded DCC frames to PNG with the act-1 palette.
//!
//! ```text
//! dcc_render <file.dcc> <out.png> [--pal FILE] [--dir N] [--frame N]
//!             [--scale S] [--bg RRGGBB]
//! dcc_render compose <file.COF> <out.png> [--pal FILE] [--dir N]
//!             [--scale S] [--bg RRGGBB]
//!             (--layer TOK=FILE.dcc ... | --dcc-dir DIR [--stem STEM])
//!             [--frames K]
//! ```
//!
//! * Single mode: decodes one DCC direction frame and colors it.
//! * Compose mode: stacks the COF layers per the COF per-frame draw order
//!   (first = bottom). With `--frames K > 1` the first K frames are laid
//!   out in a 4-column grid — the walk cycle.
//!
//! Palette triples are BGR on disk (`evidence/DCC-SPEC.md`); index 0 is
//! transparent. Output is RGBA unless `--bg RRGGBB` fills the background.

#[path = "../modules/png_mini.rs"]
#[allow(dead_code)] // write_png_rgb stays available for RGB-only callers
mod png_mini;

use d2dcc::{dir64_to_dcc, render, Cof, Dcc};

struct Args {
    pal: String,
    dir: usize,
    frame: usize,
    frames: usize,
    scale: u32,
    bg: Option<[u8; 3]>,
    layers: Vec<(String, String)>,
    dcc_dir: Option<String>,
    stem: String,
}

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if argv.len() < 2 {
        die(USAGE);
    }
    let (mode, rest) = match argv[0].as_str() {
        "compose" => (Mode::Compose, &argv[1..]),
        _ => (Mode::Single, &argv[..]),
    };
    if rest.is_empty() {
        die(USAGE);
    }
    let input = rest[0].clone();
    let out = rest[1].clone();
    let args = parse_args(&rest[2..]);

    let raw_pal = std::fs::read(&args.pal).unwrap_or_else(|e| die(format!("{}: {e}", args.pal)));
    let pal = render::Palette::load(&raw_pal, true); // BGR on disk

    let (w, h, rgba) = match mode {
        Mode::Single => render_single(&input, &pal, &args),
        Mode::Compose => render_compose(&input, &pal, &args),
    };

    let (w, h, rgba) = scale_rgba(w, h, &rgba, args.scale);
    let (w, h, rgba) = match args.bg {
        Some([r, g, b]) => {
            let mut v = rgba;
            for px in v.chunks_exact_mut(4) {
                if px[3] == 0 {
                    px[0] = r;
                    px[1] = g;
                    px[2] = b;
                    px[3] = 255;
                }
            }
            (w, h, v)
        }
        None => (w, h, rgba),
    };

    png_mini::write_png_rgba(&out, w, h, &rgba).unwrap_or_else(|e| die(format!("{out}: {e}")));
    eprintln!("wrote {out} ({w}x{h})");
}

enum Mode {
    Single,
    Compose,
}

const USAGE: &str = "usage:
  dcc_render <file.dcc> <out.png> [--pal FILE] [--dir N] [--frame N] [--scale S] [--bg RRGGBB]
  dcc_render compose <file.COF> <out.png> [--pal FILE] [--dir N] [--scale S] [--bg RRGGBB]
        (--layer TOK=FILE.dcc ... | --dcc-dir DIR [--stem STEM]) [--frames K]";

fn parse_args(rest: &[String]) -> Args {
    let mut args = Args {
        pal: String::new(),
        dir: 0,
        frame: 0,
        frames: 1,
        scale: 1,
        bg: None,
        layers: Vec::new(),
        dcc_dir: None,
        stem: String::new(),
    };
    let mut i = 0;
    while i < rest.len() {
        let flag = rest[i].clone();
        i += 1;
        let mut next = || -> String {
            let v = rest.get(i).cloned().unwrap_or_default();
            i += 1;
            v
        };
        match flag.as_str() {
            "--pal" => args.pal = next(),
            "--dir" => args.dir = next().parse().unwrap_or_else(|e| die(format!("--dir: {e}"))),
            "--frame" => {
                args.frame = next().parse().unwrap_or_else(|e| die(format!("--frame: {e}")))
            }
            "--frames" => {
                args.frames = next().parse().unwrap_or_else(|e| die(format!("--frames: {e}")))
            }
            "--scale" => args.scale = next().parse().unwrap_or_else(|e| die(format!("--scale: {e}"))),
            "--bg" => {
                let v = next();
                if v.len() != 6 {
                    die("--bg wants RRGGBB");
                }
                args.bg = Some([
                    u8::from_str_radix(&v[0..2], 16).unwrap(),
                    u8::from_str_radix(&v[2..4], 16).unwrap(),
                    u8::from_str_radix(&v[4..6], 16).unwrap(),
                ]);
            }
            "--layer" => {
                let spec = next();
                let (tok, path) = spec.split_once('=').unwrap_or_else(|| die("--layer TOK=FILE"));
                args.layers.push((tok.to_string(), path.to_string()));
            }
            "--dcc-dir" => args.dcc_dir = Some(next()),
            "--stem" => args.stem = next(),
            other => die(format!("unknown arg {other}\n{USAGE}")),
        }
    }
    if args.pal.is_empty() {
        for candidate in ["assets/map/pal_act1.dat", "assets/pal_act1.dat"] {
            if std::path::Path::new(candidate).is_file() {
                args.pal = candidate.to_string();
                break;
            }
        }
    }
    if args.pal.is_empty() {
        die("--pal FILE is required (no default palette found)");
    }
    args
}

fn load_dcc(path: &str) -> Dcc {
    let data = std::fs::read(path).unwrap_or_else(|e| die(format!("{path}: {e}")));
    Dcc::decode(&data).unwrap_or_else(|e| die(format!("{path}: {e}")))
}

fn render_single(path: &str, pal: &render::Palette, args: &Args) -> (u32, u32, Vec<u8>) {
    let dcc = load_dcc(path);
    let slot = dir64_to_dcc(args.dir, dcc.number_of_directions);
    let dir = &dcc.directions[slot];
    let f = args.frame.min(dir.frames.len().saturating_sub(1));
    let rect = render::frames_union(&[dir], f, f + 1);
    let idx = render::composite_indices(&[dir], slot, f, rect);
    eprintln!(
        "{path}: dir slot {slot} frame {f} box {:?} painted {}",
        rect,
        idx.iter().filter(|&&p| p != 0).count()
    );
    (rect.width as u32, rect.height as u32, render::indices_to_rgba(&idx, pal))
}

fn render_compose(cof_path: &str, pal: &render::Palette, args: &Args) -> (u32, u32, Vec<u8>) {
    let data = std::fs::read(cof_path).unwrap_or_else(|e| die(format!("{cof_path}: {e}")));
    let cof = Cof::parse(&data).unwrap_or_else(|e| die(format!("{cof_path}: {e}")));

    // Resolve layers: explicit --layer specs, or auto from --dcc-dir + --stem.
    let mut loaded: Vec<(String, Dcc)> = Vec::new();
    if !args.layers.is_empty() {
        for (tok, path) in &args.layers {
            loaded.push((tok.clone(), load_dcc(path)));
        }
    } else {
        let dir_root = args
            .dcc_dir
            .clone()
            .unwrap_or_else(|| die("compose needs --layer TOK=FILE ... or --dcc-dir DIR"));
        let stem = if args.stem.is_empty() {
            std::path::Path::new(cof_path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("BAWL")
                .to_string()
        } else {
            args.stem.clone()
        };
        for l in &cof.layers {
            let path = format!("{dir_root}/{stem}{}.dcc", l.layer_type.token());
            if std::path::Path::new(&path).is_file() {
                loaded.push((l.layer_type.token().to_string(), load_dcc(&path)));
            } else {
                eprintln!("note: layer {} not found at {path}, skipped", l.layer_type.token());
            }
        }
    }
    if loaded.is_empty() {
        die("no layers resolved");
    }

    let n_dirs = cof.number_of_directions;
    let n_frames = cof.frames_per_direction;
    for (tok, dcc) in &loaded {
        if dcc.number_of_directions != n_dirs || dcc.frames_per_direction != n_frames {
            die(format!(
                "layer {tok}: {}/{} dirs vs COF {}/{} — topology mismatch",
                dcc.number_of_directions, dcc.frames_per_direction, n_dirs, n_frames
            ));
        }
    }

    let slot = dir64_to_dcc(args.dir, n_dirs);
    let k = args.frames.clamp(1, n_frames);

    // Common canvas over the first K frames of every layer.
    let refs: Vec<&Dcc> = loaded.iter().map(|(_, d)| d).collect();
    let _ = refs;
    let dir_refs: Vec<&d2dcc::DccDirection> =
        loaded.iter().map(|(_, d)| &d.directions[slot]).collect();
    let rect = render::frames_union(&dir_refs, 0, k);

    // Compose each frame, then blit into the grid.
    let cols = k.min(4);
    let rows = k.div_ceil(cols);
    let sep = 1i32;
    let cw = rect.width + 2 * sep;
    let ch = rect.height + 2 * sep;
    let gw = (cols as i32) * cw + sep;
    let gh = (rows as i32) * ch + sep;
    let mut canvas = vec![0u8; (gw * gh) as usize];

    for f in 0..k {
        let order = cof.draw_order(slot, f);
        let mut ordered: Vec<&d2dcc::DccDirection> = Vec::new();
        for ct in order {
            if let Some((_, d)) = loaded.iter().find(|(tok, _)| *tok == ct.token()) {
                ordered.push(&d.directions[slot]);
            }
        }
        let idx = render::composite_indices(&ordered, slot, f, rect);
        eprintln!(
            "frame {f}: {} layers [{}] painted {}",
            ordered.len(),
            order
                .iter()
                .map(|t| t.token())
                .collect::<Vec<_>>()
                .join(","),
            idx.iter().filter(|&&p| p != 0).count()
        );
        let col = f % cols;
        let row = f / cols;
        let ox = sep + (col as i32) * cw + sep;
        let oy = sep + (row as i32) * ch + sep;
        for y in 0..rect.height {
            for x in 0..rect.width {
                let p = idx[(x as usize) + (y as usize) * rect.width as usize];
                if p != 0 {
                    canvas[(ox + x + (oy + y) * gw) as usize] = p;
                }
            }
        }
    }

    (
        gw as u32,
        gh as u32,
        render::indices_to_rgba(&canvas, pal),
    )
}

fn scale_rgba(w: u32, h: u32, rgba: &[u8], s: u32) -> (u32, u32, Vec<u8>) {
    if s <= 1 {
        return (w, h, rgba.to_vec());
    }
    let (ow, oh) = (w * s, h * s);
    let mut out = vec![0u8; (ow * oh * 4) as usize];
    for y in 0..oh {
        for x in 0..ow {
            let src = ((y / s) as usize * w as usize + (x / s) as usize) * 4;
            out[(y * ow + x) as usize * 4..(y * ow + x) as usize * 4 + 4]
                .copy_from_slice(&rgba[src..src + 4]);
        }
    }
    (ow, oh, out)
}

fn die(msg: impl std::fmt::Display) -> ! {
    eprintln!("dcc_render: {msg}");
    std::process::exit(1);
}
