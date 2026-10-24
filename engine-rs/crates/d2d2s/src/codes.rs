//! Codes 3 lettres des tables v1.12 RÉELLES (d2w-integ/assets/) — classification
//! type-spécifique des items du .d2s (evidence.md §8.5).
//!
//! * `ARMOR_CODES` : colonne `code` de `patch_armor.txt` (202 codes).
//! * `WEAPON_CODES` : colonne `code` de `patch_weapons.txt` (306 codes).
//!
//! Embarqués en compile-time (style maison d2missiles : zéro IO runtime).
//! Les codes hors de ces listes (misc : anneaux, potions, bijoux…) n'ont pas
//! de champs défense/durabilité ; les piles connues sont dans `STACKABLE_HINTS`
//! (hardcodé, cf. parseur de référence `d2s_parse.py`).

#![allow(dead_code)]

/// Trié — recherche dichotomique.
pub const ARMOR_CODES: &[&str] = &[
    "aar", "ba1", "ba2", "ba3", "ba4", "ba5", "ba6", "ba7", "ba8", "ba9",
    "baa", "bab", "bac", "bad", "bae", "baf", "bhm", "brs", "bsh", "buc",
    "cap", "chn", "ci0", "ci1", "ci2", "ci3", "crn", "dr1", "dr2", "dr3",
    "dr4", "dr5", "dr6", "dr7", "dr8", "dr9", "dra", "drb", "drc", "drd",
    "dre", "drf", "fhl", "fld", "ful", "ghm", "gth", "gts", "hbl", "hbt",
    "hgl", "hla", "hlm", "kit", "lbl", "lbt", "lea", "lgl", "lrg", "ltp",
    "mbl", "mbt", "mgl", "msk", "ne1", "ne2", "ne3", "ne4", "ne5", "ne6",
    "ne7", "ne8", "ne9", "nea", "neb", "ned", "nee", "nef", "neg", "pa1",
    "pa2", "pa3", "pa4", "pa5", "pa6", "pa7", "pa8", "pa9", "paa", "pab",
    "pac", "pad", "pae", "paf", "plt", "qui", "rng", "scl", "skp", "sml",
    "spk", "spl", "stu", "tbl", "tbt", "tgl", "tow", "uap", "uar", "ucl",
    "uea", "uh9", "uhb", "uhc", "uhg", "uhl", "uhm", "uhn", "uit", "ukp",
    "ula", "ulb", "ulc", "uld", "ulg", "ulm", "ult", "umb", "umc", "umg",
    "uml", "ung", "uow", "upk", "upl", "urg", "urn", "urs", "ush", "usk",
    "utb", "utc", "utg", "uth", "utp", "uts", "utu", "uuc", "uui", "uul",
    "uvb", "uvc", "uvg", "vbl", "vbt", "vgl", "xap", "xar", "xcl", "xea",
    "xh9", "xhb", "xhg", "xhl", "xhm", "xhn", "xit", "xkp", "xla", "xlb",
    "xld", "xlg", "xlm", "xlt", "xmb", "xmg", "xml", "xng", "xow", "xpk",
    "xpl", "xrg", "xrn", "xrs", "xsh", "xsk", "xtb", "xtg", "xth", "xtp",
    "xts", "xtu", "xuc", "xui", "xul", "xvb", "xvg", "zhb", "zlb", "zmb",
    "ztb", "zvb",
];

/// Trié — recherche dichotomique.
pub const WEAPON_CODES: &[&str] = &[
    "2ax", "2hs", "6bs", "6cb", "6cs", "6hb", "6hx", "6l7", "6lb", "6ls",
    "6lw", "6lx", "6mx", "6rx", "6s7", "6sb", "6ss", "6sw", "6ws", "72a",
    "72h", "7ar", "7ax", "7b7", "7b8", "7ba", "7bk", "7bl", "7br", "7bs",
    "7bt", "7bw", "7cl", "7cm", "7cr", "7cs", "7dg", "7di", "7fb", "7fc",
    "7fl", "7ga", "7gd", "7gi", "7gl", "7gm", "7gs", "7gw", "7h7", "7ha",
    "7ja", "7kr", "7la", "7ls", "7lw", "7m7", "7ma", "7mp", "7mt", "7o7",
    "7p7", "7pa", "7pi", "7qr", "7qs", "7s7", "7s8", "7sb", "7sc", "7sm",
    "7sp", "7sr", "7ss", "7st", "7ta", "7tk", "7tr", "7ts", "7tw", "7vo",
    "7wa", "7wb", "7wc", "7wd", "7wh", "7wn", "7ws", "7xf", "7yw", "8bs",
    "8cb", "8cs", "8hb", "8hx", "8l8", "8lb", "8ls", "8lw", "8lx", "8mx",
    "8rx", "8s8", "8sb", "8ss", "8sw", "8ws", "92a", "92h", "9ar", "9ax",
    "9b7", "9b8", "9b9", "9ba", "9bk", "9bl", "9br", "9bs", "9bt", "9bw",
    "9cl", "9cm", "9cr", "9cs", "9dg", "9di", "9fb", "9fc", "9fl", "9ga",
    "9gd", "9gi", "9gl", "9gm", "9gs", "9gw", "9h9", "9ha", "9ja", "9kr",
    "9la", "9ls", "9lw", "9m9", "9ma", "9mp", "9mt", "9p9", "9pa", "9pi",
    "9qr", "9qs", "9s8", "9s9", "9sb", "9sc", "9sm", "9sp", "9sr", "9ss",
    "9st", "9ta", "9tk", "9tr", "9ts", "9tw", "9vo", "9wa", "9wb", "9wc",
    "9wd", "9wh", "9wn", "9ws", "9xf", "9yw", "am1", "am2", "am3", "am4",
    "am5", "am6", "am7", "am8", "am9", "ama", "amb", "amc", "amd", "ame",
    "amf", "axe", "axf", "bal", "bar", "bax", "bkf", "bld", "brn", "bsd",
    "bst", "bsw", "btl", "btx", "bwn", "cbw", "ces", "clb", "clm", "clw",
    "crs", "cst", "d33", "dgr", "dir", "fla", "flb", "flc", "g33", "gax",
    "gis", "gix", "glv", "gma", "gpl", "gpm", "gps", "gsc", "gsd", "gwn",
    "hal", "hax", "hbw", "hdm", "hfh", "hst", "hxb", "jav", "kri", "ktr",
    "lax", "lbb", "lbw", "leg", "lsd", "lst", "lwb", "lxb", "mac", "mau",
    "mpi", "msf", "mst", "mxb", "ob1", "ob2", "ob3", "ob4", "ob5", "ob6",
    "ob7", "ob8", "ob9", "oba", "obb", "obc", "obd", "obe", "obf", "opl",
    "opm", "ops", "pax", "pik", "pil", "qf1", "qf2", "rxb", "sbb", "sbr",
    "sbw", "scm", "scp", "scy", "skr", "spc", "spr", "spt", "ssd", "ssp",
    "sst", "swb", "tax", "tkf", "tri", "tsp", "vou", "wax", "whm", "wnd",
    "wrb", "wsc", "wsd", "wsp", "wst", "ywn",
];

/// Codes "pile/quantité" (hardcodés comme le parseur de référence, evidence §8.5).
pub const STACKABLE_HINTS: &[&str] = &[
    "tbk", "ibk", "key", "tsc", "isc", "vps", "yps", "ww1", "ww2", "ww3", "aqv", "cqv", "xqv",
];

/// Codes tomes : 5 bits inconnus avant la quantité (trevin).
pub const TOME_CODES: &[&str] = &["tsc", "isc", "vps", "yps", "ww1", "ww2", "ww3"];

pub(crate) fn in_sorted(set: &[&str], code: &str) -> bool {
    set.binary_search(&code).is_ok()
}

/// Le code est une armure (champs défense/durabilité/sockets).
pub fn is_armor(code: &str) -> bool { in_sorted(ARMOR_CODES, code) }

/// Le code est une arme (champs durabilité/sockets).
pub fn is_weapon(code: &str) -> bool { in_sorted(WEAPON_CODES, code) }

/// Le code est une pile (quantité 9 bits).
pub fn is_stackable(code: &str) -> bool { in_sorted(STACKABLE_HINTS, code) }
