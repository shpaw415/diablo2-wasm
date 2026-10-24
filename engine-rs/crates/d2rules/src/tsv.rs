//! Parseur TSV générique runtime — socle data-driven W01/A3 (audits d2w-re).
//!
//! Les tables `.txt` de Diablo 2 v1.12 sont des TSV Excel exportés. Format
//! réel constaté sur les tables extraites (`d2w-re/tables/**` et
//! `d2w-integ/assets/**`, 2026-10-24) :
//!
//! * **CRLF** : lignes terminées `\r\n` (ex. `monai.txt`, `superuniques.txt`) ;
//!   le parseur accepte aussi LF seul, et une dernière ligne sans terminaison.
//! * **Tabulations** : séparateur `\t` ; une ligne finissant par `\t` porte
//!   une dernière cellule vide ; une ligne plus courte que l'en-tête est
//!   complétée par des cellules vides (accès lenient).
//! * **Quotes Excel** : un champ peut être enveloppé de `"` (ex. `shrines.txt`
//!   ligne 5 : `"doubles current mana ("") (OUT)"`) ; à l'intérieur, `""` est
//!   une quote littérale. Une quote hors début de champ reste littérale.
//! * **En-têtes nommés** : première ligne = noms de colonnes. Les colonnes
//!   préfixées `*` (`*eol`, `*aip1` de `monai.txt`) sont des colonnes
//!   commentaires D2 — traitées comme les autres. Les noms peuvent être
//!   **dupliqués** (`AutoMap.txt` porte deux colonnes `Type2`) : la première
//!   occurrence gagne.
//! * Les lignes entièrement vides sont du bruit et sont sautées.
//!
//! Zéro IO : le texte arrive en `&str` (embed `include_str!` ou asset runtime).
//! Zéro panique : les erreurs remontent en [`TsvError`], les accès manquants
//! retournent `""`/`0` (même sémantique lenient que `d2game::levels::Row`).

/// Erreur de parse (aujourd'hui : table sans en-tête). `line` = ligne 1-based.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TsvError {
    /// Ligne (1-based) où l'erreur a été détectée.
    pub line: usize,
    pub message: String,
}

impl std::fmt::Display for TsvError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "tsv ligne {} : {}", self.line, self.message)
    }
}

impl std::error::Error for TsvError {}

/// Une table TSV parsée : colonnes nommées + rangées de cellules.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tsv {
    columns: Vec<String>,
    rows: Vec<Vec<String>>,
}

impl Tsv {
    /// Parse un texte TSV complet. La première ligne non vide est l'en-tête ;
    /// les rangées suivantes sont les records (lignes vides sautées, lignes
    /// courtes complétées, cellules superflues au-delà de l'en-tête gardées).
    pub fn parse(input: &str) -> Result<Tsv, TsvError> {
        let bytes = input.as_bytes();
        let mut cursor = 0usize;
        let mut line = 1usize;
        let mut header = loop {
            match next_record(bytes, &mut cursor, &mut line) {
                None => {
                    return Err(TsvError { line: 1, message: "table vide (aucun en-tête)".into() })
                }
                Some(rec) if is_blank(&rec) => continue, // bruit d'entrée
                Some(rec) => break rec,
            }
        };
        let mut rows = Vec::new();
        while let Some(mut rec) = next_record(bytes, &mut cursor, &mut line) {
            if is_blank(&rec) {
                continue; // ligne vide (bruit) — cf. doc module
            }
            // Ligne plus courte que l'en-tête → cellules "" implicites ;
            // plus longue → cellules superflues conservées (accès par index).
            let width = header.len();
            if rec.len() < width {
                rec.resize(width, String::new());
            }
            rows.push(rec);
        }
        let columns = header.iter().map(|c| c.trim().to_string()).collect();
        Ok(Tsv { columns, rows })
    }

    /// Noms de colonnes (noms dupliqués possibles — [`Tsv::column`] prend le
    /// premier).
    pub fn columns(&self) -> &[String] {
        &self.columns
    }

    /// Nombre de records (hors en-tête, hors lignes vides).
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Cellules brutes du record `row` (peut dépasser l'en-tête).
    pub fn row(&self, row: usize) -> Option<&[String]> {
        self.rows.get(row).map(|r| r.as_slice())
    }

    /// Index de la colonne nommée — PREMIÈRE occurrence (AutoMap.txt porte
    /// deux colonnes `Type2`).
    pub fn column(&self, name: &str) -> Option<usize> {
        self.columns.iter().position(|c| c == name)
    }

    /// Cellule brute `(record, colonne nommée)` — `None` si le record manque,
    /// `Some("")` si la colonne manque.
    pub fn cell(&self, row: usize, col: &str) -> Option<&str> {
        let rec = self.rows.get(row)?;
        Some(
            rec.get(self.column(col)?)
                .map(|s| s.as_str())
                .unwrap_or(""),
        )
    }

    /// Accesseur typé lenient sur le record `row`.
    pub fn rec(&self, row: usize) -> Option<TsvRec<'_>> {
        if row < self.rows.len() {
            Some(TsvRec { tsv: self, row })
        } else {
            None
        }
    }
}

fn is_blank(rec: &[String]) -> bool {
    rec.iter().all(|c| c.trim().is_empty())
}

/// Accesseur lenient d'un record : cellule manquante ⇒ `""` / `0`.
#[derive(Debug, Clone, Copy)]
pub struct TsvRec<'a> {
    tsv: &'a Tsv,
    row: usize,
}

impl<'a> TsvRec<'a> {
    /// Cellule trimmée (`""` si colonne/ligne absente).
    pub fn s(&self, col: &str) -> &'a str {
        self.tsv
            .cell(self.row, col)
            .unwrap_or("")
            .trim()
    }

    /// Entier lenient : `""` / absent / illisible ⇒ 0.
    pub fn i(&self, col: &str) -> i32 {
        self.s(col).parse::<i32>().unwrap_or(0)
    }

    /// Entier non signé lenient (négatif ⇒ 0).
    pub fn u(&self, col: &str) -> u32 {
        self.i(col).max(0) as u32
    }
}

/// Lit le prochain record à partir de `cursor` — `None` en fin d'entrée.
/// Gère CRLF/LF, champs quotés Excel (`""` = quote littérale, tab et fin de
/// ligne à l'intérieur des quotes littéraux), dernière ligne non terminée.
fn next_record(bytes: &[u8], cursor: &mut usize, line: &mut usize) -> Option<Vec<String>> {
    if *cursor >= bytes.len() {
        return None;
    }
    let mut fields = Vec::new();
    let mut cur = String::new();
    loop {
        if *cursor >= bytes.len() {
            fields.push(cur);
            return Some(fields);
        }
        match bytes[*cursor] {
            b'"' if cur.is_empty() => {
                // Champ quoté Excel : jusqu'à la quote fermante, `""` = `"`
                // littérale ; une quote non fermée est tolérée (fin d'entrée
                // = fin de champ — données de jeu, jamais paniquer).
                *cursor += 1;
                loop {
                    if *cursor >= bytes.len() {
                        fields.push(cur);
                        return Some(fields);
                    }
                    match bytes[*cursor] {
                        b'"' if bytes.get(*cursor + 1) == Some(&b'"') => {
                            cur.push('"');
                            *cursor += 2;
                        }
                        b'"' => {
                            *cursor += 1;
                            break;
                        }
                        b'\n' => {
                            cur.push('\n');
                            *line += 1;
                            *cursor += 1;
                        }
                        _ => push_char(bytes, cursor, &mut cur),
                    }
                }
                // Après la quote fermante : `\t` / CRLF / EOF attendus ;
                // tout autre octet est recollé au champ (lenient).
            }
            b'\t' => {
                fields.push(std::mem::take(&mut cur));
                *cursor += 1;
            }
            b'\r' if bytes.get(*cursor + 1) == Some(&b'\n') => {
                *cursor += 2;
                *line += 1;
                fields.push(cur);
                return Some(fields);
            }
            b'\n' => {
                *cursor += 1;
                *line += 1;
                fields.push(cur);
                return Some(fields);
            }
            _ => push_char(bytes, cursor, &mut cur),
        }
    }
}

/// Avance d'un caractère UTF-8 complet (les octets de contrôle ASCII ciblés
/// par le parseur ne peuvent pas apparaître dans une suite multi-octets).
fn push_char(bytes: &[u8], cursor: &mut usize, out: &mut String) {
    let b = bytes[*cursor];
    let len = match b {
        0x00..=0x7F => 1,
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        0xF0..=0xF7 => 4,
        _ => 1, // octet de continuation isolé : remplacé
    };
    let end = (*cursor + len).min(bytes.len());
    match std::str::from_utf8(&bytes[*cursor..end]) {
        Ok(s) => out.push_str(s),
        Err(_) => out.push('\u{FFFD}'),
    }
    *cursor = end;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crlf_lf_and_last_line_without_terminator() {
        // Format réel : CRLF partout (ex. monai.txt) — LF seul et dernière
        // ligne sans terminaison acceptés aussi.
        let t = Tsv::parse("a\tb\r\nc\td\n last\tno_eol").unwrap();
        assert_eq!(t.len(), 2, "2 records (l'en-tête ne compte pas)");
        assert_eq!(t.cell(0, "a"), Some("c"));
        assert_eq!(t.cell(1, "b"), Some("no_eol"));
    }

    #[test]
    fn quoted_field_with_doubled_quotes() {
        // shrines.txt ligne 5 : `"doubles current mana ("") (OUT)"` → la
        // cellule contient `doubles current mana (") (OUT)`.
        let t = Tsv::parse("name\teffect\r\nMana Boost\t\"doubles current mana (\"\") (OUT)\"\r\n")
            .unwrap();
        assert_eq!(t.len(), 1);
        assert_eq!(t.cell(0, "effect"), Some("doubles current mana (\") (OUT)"));
    }

    #[test]
    fn quoted_field_keeps_tabs_and_newlines_literal() {
        let t = Tsv::parse("a\tb\r\nx\t\"tab\there nl\nthere\"\r\ny\tz\r\n").unwrap();
        assert_eq!(t.len(), 2);
        assert_eq!(t.cell(0, "b"), Some("tab\there nl\nthere"));
        assert_eq!(t.cell(1, "a"), Some("y"));
    }

    #[test]
    fn trailing_tab_is_empty_last_cell() {
        let t = Tsv::parse("a\tb\r\nx\t\r\n").unwrap();
        assert_eq!(t.cell(0, "b"), Some(""));
    }

    #[test]
    fn short_row_padded_long_row_keeps_extras() {
        let t = Tsv::parse("a\tb\tc\r\nshort\t\none\ttwo\tthree\tfour\r\n").unwrap();
        assert_eq!(t.len(), 2);
        assert_eq!(t.cell(0, "c"), Some(""), "ligne courte ⇒ cellule vide");
        assert_eq!(t.row(1).unwrap().len(), 4, "cellule superflue conservée");
    }

    #[test]
    fn duplicate_columns_first_occurrence_wins() {
        // AutoMap.txt : deux colonnes `Type2` dans l'en-tête réel.
        let t = Tsv::parse("x\tType2\tType2\r\nr\tA\tB\r\n").unwrap();
        assert_eq!(t.column("Type2"), Some(1), "première occurrence");
        assert_eq!(t.cell(0, "Type2"), Some("A"));
    }

    #[test]
    fn blank_lines_skipped_and_star_columns_are_normal_columns() {
        // monai.txt : colonnes *aip1..*aip8 / *eol = colonnes commentaires D2,
        // adressables comme les autres.
        let t = Tsv::parse("AI\t*eol\r\n\r\nNone\t0\r\n\r\nIdle\t0\r\n").unwrap();
        assert_eq!(t.len(), 2);
        assert_eq!(t.cell(0, "*eol"), Some("0"));
        assert_eq!(t.cell(1, "AI"), Some("Idle"));
    }

    #[test]
    fn header_names_are_trimmed_cells_are_trimmed() {
        let t = Tsv::parse("id \tspaced\r\n  r1 \tv\t\r\n").unwrap();
        assert_eq!(t.column("id"), Some(0));
        assert_eq!(t.cell(0, "spaced"), Some("v"));
    }

    #[test]
    fn lenient_numeric_accessors() {
        let t = Tsv::parse("n\tneg\tempty\r\n42\t-7\t\r\n").unwrap();
        let r = t.rec(0).unwrap();
        assert_eq!(r.u("n"), 42);
        assert_eq!(r.i("neg"), -7);
        assert_eq!(r.u("neg"), 0, "négatif ⇒ 0");
        assert_eq!(r.u("empty"), 0);
        assert_eq!(r.u("absente"), 0);
        assert_eq!(r.s("absente"), "");
    }

    #[test]
    fn empty_input_is_an_error() {
        assert!(Tsv::parse("").is_err());
        assert!(Tsv::parse("\r\n\r\n").is_err(), "uniquement des lignes vides");
        let err = Tsv::parse("").unwrap_err();
        assert!(err.to_string().contains("tsv"));
    }

    #[test]
    fn lone_quote_and_lone_cr_are_literal() {
        // Quote au milieu d'un champ non quoté : littérale ; `\r` isolé (sans
        // LF) : simple caractère, pas une fin de ligne.
        let t = Tsv::parse("a\r\n5'9\" ok\rx\r\n").unwrap();
        assert_eq!(t.len(), 1);
        assert!(t.cell(0, "a").unwrap().contains('"'));
        assert!(t.cell(0, "a").unwrap().contains('\r'));
    }

    #[test]
    fn utf8_cells_survive() {
        let t = Tsv::parse("nom\r\nChâteau d'Éguard\r\n").unwrap();
        assert_eq!(t.cell(0, "nom"), Some("Château d'Éguard"));
    }

    // Le golden réel ci-dessous verrouille le comportement sur une vraie
    // table quotée (shrines.txt, 23 records + effets Excel `""`).
    #[test]
    fn real_shrines_table_unquotes_excel_fields() {
        const SHRINES: &str = include_str!("../../../../../d2w-re/tables/monde/shrines.txt");
        let t = Tsv::parse(SHRINES).unwrap();
        assert_eq!(t.len(), 23, "shrines.txt : 23 records réels");
        // ligne 5 du fichier (idx 3) : effet quoté avec `""` → quote littérale.
        assert_eq!(
            t.cell(3, "Effect"),
            Some("doubles current mana (\") (OUT)"),
            "shrines.txt ligne 5 (Mana Boost) : déquotage Excel"
        );
    }
}
