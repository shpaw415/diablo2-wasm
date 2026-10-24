//! Parseur/évaluateur `CalcString` D2 — port direct de
//! `opendiablo2/d2common/d2calculation` ({calculation,calcstring}.go) +
//! `d2lexer/lexer.go` + `d2parser/{parser,operations}.go` (socle A2 de
//! `finish-systemes.md`). Pilote toutes les colonnes `*Calc` de skills.txt :
//! synergies, ToHitCalc, passives, auras.
//!
//! # Sémantique (calquée sur le Go, écarts documentés)
//! * Arithmétique **entière 64 bits** (`/` tronque vers zéro comme Go) ;
//!   division par zéro ⇒ 0 (écart : Go panique, le port est sans panic).
//! * Opérateurs binaires : `== !=` (prio 1), `< > <= >=` (2), `+ -` (3),
//!   `* /` (5), `^` (6, associatif à droite) ; unaire `+ -` (4) ; ternaire
//!   `?:` (0, associatif à droite) ; fonctions fixes `min(a,b)`, `max(a,b)`,
//!   `rand(a,b)` — via [`EvalContext::random`] pour rester déterministe.
//! * Propriétés : `skill('X'.qual)`, `stat('X'.qual)`, `miss('X'.qual)` — la
//!   forme réelle v1.12 met le `.qual` DANS les parenthèses (`skill('Fire
//!   Ball'.blvl)`), la forme OD2 hors parenthèses est aussi acceptée.
//!   Qualificateurs réels v1.12 (patch_skills.txt) : `blvl` (269×), `lvl`,
//!   `parN`/`lnXX`/`dmXX` (paires Param du record référencé, ex.
//!   `skill('Golem Mastery'.dm34)` ligne Clay Golem) et les raccourcis d'aura
//!   `edmn`/`edmx`/`edns` (courbes EMin/EMax/ELen du référencé, ex.
//!   `skill('Prayer'.edns)` ligne Cleansing) — résolus côté
//!   [`crate::skills_tree`].
//! * Noms nus : `lvl` (niveau du skill), `ulvl`, `par1`..`par8`, `lnXX`/`dmXX`
//!   (paires Param linéaires : `lnAB` = `ParamA + (niveau-1) × ParamB`) et les
//!   raccourcis `edmn`/`edmx`/`edns` (courbes élémentaires du record
//!   propriétaire — réels : `min(edmn,95)` ligne Energy Shield, `-edmn` ligne
//!   Inner Sight, `edns` ligne Prayer) résolus par [`EvalContext::bare`] ;
//!   nom inconnu ⇒ 1 (comportement placeholder du Go).
//! * Bindings `stat(...)` : [`StatListCtx`] résout `stat('id'.q)` par id
//!   numérique **et** `stat('nom'.q)` via le dictionnaire
//!   [`crate::stats::StatCostTable`] sur une [`crate::statlist::StatList`] par
//!   ids — y compris les stats « charge/quantité » d'items v1.12
//!   (`item_charged_skill` id 204, `quantity` id 70).
//! * Chaîne vide ⇒ constante 0 ; erreur de parse ⇒ évaluation 0, jamais de
//!   panic (écart : Go `panic!`+log).

use std::fmt;

/// Erreur de lexing/parsing d'un CalcString (position en octets).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalcError {
    /// Position en octets dans la source.
    pub pos: usize,
    /// Description courte.
    pub msg: String,
}

impl fmt::Display for CalcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "calcstring: {} (octet {})", self.msg, self.pos)
    }
}

impl std::error::Error for CalcError {}

/// Type de token — calque `d2lexer` Go (`Name`, `String`, `Symbol`, `Number`, EOF).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TokKind {
    /// Identifiant alphanumérique (`skill`, `lvl`, `min`…).
    Name,
    /// Chaîne entre apostrophes (`'Fire Ball'`).
    Str,
    /// Symbole (`+`, `(`, `==`…).
    Symbol,
    /// Entier littéral.
    Number,
    /// Fin d'entrée.
    Eof,
}

/// Token lexical.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Token {
    pub(crate) kind: TokKind,
    pub(crate) value: String,
    /// Position en octets du début du token.
    pub(crate) pos: usize,
}

/// Lexer — port de `d2lexer/lexer.go` (mêmes classes de caractères : opérateurs
/// `^=!><+-/*.,:?()`, chiffres, apostrophes, lettres/chiffres ASCII).
pub(crate) struct Lexer {
    data: Vec<u8>,
    index: usize,
    peeked: Option<Token>,
}

impl Lexer {
    pub(crate) fn new(input: &str) -> Lexer {
        Lexer { data: input.as_bytes().to_vec(), index: 0, peeked: None }
    }

    fn peek_byte(&self) -> Option<u8> {
        self.data.get(self.index + 1).copied()
    }

    fn sym(&mut self) -> Token {
        let pos = self.index;
        let c = self.data[self.index];
        if c == b'=' || c == b'!' {
            // Go exige '=' ou '!' suivi de '=' sinon panic ; on produit une
            // erreur propre via un token symbole isolé (le parseur la remonte).
            if self.peek_byte() == Some(b'=') {
                self.index += 2;
                let v = format!("{}=", c as char);
                return Token { kind: TokKind::Symbol, value: v, pos };
            }
        } else if (c == b'<' || c == b'>') && self.peek_byte() == Some(b'=') {
            self.index += 2;
            let v = format!("{}=", c as char);
            return Token { kind: TokKind::Symbol, value: v, pos };
        }
        self.index += 1;
        Token { kind: TokKind::Symbol, value: (c as char).to_string(), pos }
    }

    fn number(&mut self) -> Token {
        let pos = self.index;
        let start = self.index;
        while self.index < self.data.len() && self.data[self.index].is_ascii_digit() {
            self.index += 1;
        }
        let value = String::from_utf8_lossy(&self.data[start..self.index]).into_owned();
        Token { kind: TokKind::Number, value, pos }
    }

    fn string(&mut self) -> Token {
        let pos = self.index;
        self.index += 1; // apostrophe ouvrante
        let start = self.index;
        while self.index < self.data.len() && self.data[self.index] != b'\'' {
            self.index += 1;
        }
        let value = String::from_utf8_lossy(&self.data[start..self.index]).into_owned();
        self.index = (self.index + 1).min(self.data.len()); // apostrophe fermante
        Token { kind: TokKind::Str, value, pos }
    }

    fn name(&mut self) -> Token {
        let pos = self.index;
        let start = self.index;
        while self.index < self.data.len()
            && (self.data[self.index].is_ascii_alphanumeric())
        {
            self.index += 1;
        }
        let value = String::from_utf8_lossy(&self.data[start..self.index]).into_owned();
        Token { kind: TokKind::Name, value, pos }
    }

    /// Prochain token sans avancer (port de `Peek`).
    pub(crate) fn peek(&mut self) -> Token {
        if let Some(t) = &self.peeked {
            return t.clone();
        }
        let t = self.scan();
        self.peeked = Some(t.clone());
        t
    }

    /// Consomme et renvoie le prochain token (port de `NextToken`).
    pub(crate) fn next_token(&mut self) -> Token {
        match self.peeked.take() {
            Some(t) => t,
            None => self.scan(),
        }
    }

    fn scan(&mut self) -> Token {
        let eof = Token { kind: TokKind::Eof, value: String::new(), pos: self.index };
        if self.index == self.data.len() {
            return eof;
        }
        while self.index < self.data.len() && self.data[self.index].is_ascii_whitespace() {
            self.index += 1;
        }
        if self.index == self.data.len() {
            return Token { kind: TokKind::Eof, value: String::new(), pos: self.index };
        }
        let c = self.data[self.index];
        if b"^=!><+-/*.,:?()".contains(&c) {
            self.sym()
        } else if c.is_ascii_digit() {
            self.number()
        } else if c == b'\'' {
            self.string()
        } else if c.is_ascii_alphabetic() {
            self.name()
        } else {
            // Go panique (« Invalid token ») ; token erreur consommable.
            self.index += 1;
            Token { kind: TokKind::Symbol, value: "\u{0}".into(), pos: self.index - 1 }
        }
    }
}

/// Contexte d'évaluation — résout les références données d'un CalcString.
/// L'implémenteur porte la vue « skill courant + héros » (voir
/// [`crate::skills_tree::SkillEvalCtx`]). Par défaut : inconnu ⇒ 0 pour les
/// skills/stats, 1 pour missile/nus (placeholder Go), `random` ⇒ `v1`.
pub trait EvalContext {
    /// `skill('X'.qual)` — `qual` = `blvl` (points investis), `lvl` (total
    /// avec +skills), autre ⇒ 0 si le skill est inconnu du contexte.
    fn skill_level(&mut self, _skill: &str, _qualifier: &str) -> i64 {
        0
    }
    /// `stat('X'.qual)` — valeur d'une stat (`val`, `accr`…).
    fn stat_value(&mut self, _stat: &str, _qualifier: &str) -> i64 {
        0
    }
    /// `miss('X'.qual)` — propriété d'un missile.
    fn missile_prop(&mut self, _missile: &str, _qualifier: &str) -> i64 {
        1
    }
    /// Nom nu : `lvl`/`sklvl` (niveau du skill courant), `ulvl` (niveau de
    /// l'unité), `par1..8` (Param du skill courant), `lnXX`/`dmXX` (paires),
    /// autre ⇒ 1 (placeholder Go).
    fn bare(&mut self, _name: &str) -> i64 {
        1
    }
    /// `rand(v1, v2)` — choix déterministe requis (contrat RNG d2rules).
    /// Défaut : renvoie `v1` (déterministe).
    fn random(&mut self, v1: i64, _v2: i64) -> i64 {
        v1
    }
}

/// Contexte nul : tout résout aux défauts (tests, formules sans données).
pub struct NullCtx;

impl EvalContext for NullCtx {}

/// Contexte branché sur une [`crate::statlist::StatList`] par ids
/// `ItemStatCost` — le binding `stat(...)` complet :
///
/// * `stat('123'.val)` — id numérique direct (langage `stat.id` des saves
///   binaires) ;
/// * `stat('strength'.val)` — nom du dictionnaire [`crate::stats::StatCostTable`]
///   → id → valeur ;
/// * stats « charge/quantité » d'items v1.12 : `item_charged_skill` (id 204),
///   `quantity` (id 70) — mêmes chemins ;
/// * repli [`crate::stats::StatList`] par noms (héros v2) pour un nom absent
///   du dictionnaire.
///
/// Chaîne de résolution (documentée, déterministe) : id numérique →
/// dictionnaire → repli noms ; inconnu partout ⇒ 0 (jamais de panic). Les
/// qualifications (`.val`, `.accr` — les deux seules de v1.12 : Hydra
/// `stat('passive_fire_mastery'.accr)`) lisent la même valeur.
pub struct StatListCtx<'a> {
    /// Stats agrégées par id `ItemStatCost`.
    pub ids: &'a crate::statlist::StatList,
    /// Repli : liste par noms (héros v2), consultée si nom absent du
    /// dictionnaire.
    pub names: Option<&'a crate::stats::StatList>,
}

impl EvalContext for StatListCtx<'_> {
    fn stat_value(&mut self, stat: &str, _qualifier: &str) -> i64 {
        // 1) Id numérique direct (saves binaires : `stat('0'.val)` = strength).
        if let Ok(id) = stat.trim().parse::<i32>() {
            return self.ids.get(id);
        }
        // 2) Nom → dictionnaire → id.
        if let Some(id) = crate::stats::StatCostTable::global().get(stat).map(|r| r.id) {
            return self.ids.get(id);
        }
        // 3) Repli liste par noms (héros v2).
        self.names.map(|s| s.get(stat)).unwrap_or(0)
    }
    fn skill_level(&mut self, _skill: &str, _qualifier: &str) -> i64 {
        0
    }
    fn missile_prop(&mut self, _missile: &str, _qualifier: &str) -> i64 {
        1
    }
    fn bare(&mut self, _name: &str) -> i64 {
        1
    }
}

/// AST évaluable — port de `d2calculation` Go (Binary/Unary/Ternary/Property/
/// Constant ; les noms nus deviennent [`Node::Bare`]).
#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    /// Constante entière.
    Constant(i64),
    /// Binaire : `(op, gauche, droite)`.
    Binary(BinOp, Box<Node>, Box<Node>),
    /// Unaire : `(op, enfant)`.
    Unary(UnOp, Box<Node>),
    /// Ternaire `cond ? alors : sinon` (alors évalué paresseusement).
    Ternary(Box<Node>, Box<Node>, Box<Node>),
    /// Référence `skill('X'.q)` / `stat('X'.q)` / `miss('X'.q)`.
    Prop(PropKind, String, String),
    /// Nom nu (`lvl`, `par3`, `ln12`, `dm34`, `ulvl`…).
    Bare(String),
    /// Fonction fixe `min`/`max` (deux arguments).
    FixedFn(FixedFn, Box<Node>, Box<Node>),
}

/// Opérateurs binaires (priorités identiques au Go : voir module doc).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    /// `==`
    Eq,
    /// `!=`
    Ne,
    /// `<`
    Lt,
    /// `>`
    Gt,
    /// `<=`
    Le,
    /// `>=`
    Ge,
    /// `+`
    Add,
    /// `-`
    Sub,
    /// `*`
    Mul,
    /// `/` (tronque vers zéro ; ÷0 ⇒ 0)
    Div,
    /// `^` (associatif à droite, saturant)
    Pow,
}

/// Opérateurs unaires.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    /// `+x`
    Pos,
    /// `-x`
    Neg,
}

/// Type de référence de propriété.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropKind {
    /// `skill('X'.q)`
    Skill,
    /// `stat('X'.q)`
    Stat,
    /// `miss('X'.q)`
    Missile,
}

/// Fonctions fixes à deux arguments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixedFn {
    /// `min(a, b)`
    Min,
    /// `max(a, b)`
    Max,
    /// `rand(a, b)` — via [`EvalContext::random`] (déterministe).
    Rand,
}

impl BinOp {
    /// Priorité du Go (`operations.go`) : 1..=6.
    fn precedence(self) -> u8 {
        match self {
            BinOp::Eq | BinOp::Ne => 1,
            BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => 2,
            BinOp::Add | BinOp::Sub => 3,
            BinOp::Mul | BinOp::Div => 5,
            BinOp::Pow => 6,
        }
    }
    /// Associativité (`^` à droite).
    fn right_assoc(self) -> bool {
        self == BinOp::Pow
    }
    fn from_symbol(s: &str) -> Option<BinOp> {
        Some(match s {
            "==" => BinOp::Eq,
            "!=" => BinOp::Ne,
            "<" => BinOp::Lt,
            ">" => BinOp::Gt,
            "<=" => BinOp::Le,
            ">=" => BinOp::Ge,
            "+" => BinOp::Add,
            "-" => BinOp::Sub,
            "*" => BinOp::Mul,
            "/" => BinOp::Div,
            "^" => BinOp::Pow,
            _ => return None,
        })
    }
    fn apply(self, a: i64, b: i64) -> i64 {
        match self {
            BinOp::Eq => (a == b) as i64,
            BinOp::Ne => (a != b) as i64,
            BinOp::Lt => (a < b) as i64,
            BinOp::Gt => (a > b) as i64,
            BinOp::Le => (a <= b) as i64,
            BinOp::Ge => (a >= b) as i64,
            BinOp::Add => a.wrapping_add(b),
            BinOp::Sub => a.wrapping_sub(b),
            BinOp::Mul => a.wrapping_mul(b),
            // Go : division entière (panique ÷0) ; port : 0, sans panic.
            BinOp::Div => {
                if b == 0 {
                    0
                } else {
                    a.wrapping_div(b)
                }
            }
            // Go : math.Pow flottant ; port : pow entier saturant.
            BinOp::Pow => {
                if b < 0 {
                    0
                } else {
                    a.checked_pow(b.min(u32::MAX as i64) as u32).unwrap_or_else(|| {
                        if a < 0 && b % 2 == 1 {
                            i64::MIN
                        } else {
                            i64::MAX
                        }
                    })
                }
            }
        }
    }
}

/// Parseur — port de `d2parser/parser.go` (descente par priorités, boucle
/// binaire puis boucle ternaire, production parenthèse/unaire/feuille).
struct Parser {
    lex: Lexer,
}

impl Parser {
    fn peek(&mut self) -> Token {
        self.lex.peek()
    }
    fn consume(&mut self) -> Token {
        self.lex.next_token()
    }

    /// Port de `parseLevel` : expression au niveau de priorité `level`.
    fn parse_level(&mut self, level: u8) -> Result<Node, CalcError> {
        let mut node = self.parse_production()?;

        // Boucle binaire (priorités croissantes).
        loop {
            let t = self.peek();
            if t.kind != TokKind::Symbol {
                break;
            }
            let Some(op) = BinOp::from_symbol(&t.value) else { break };
            if op.precedence() < level {
                break;
            }
            self.consume();
            let next = if op.right_assoc() { op.precedence() } else { op.precedence() + 1 };
            let rhs = self.parse_level(next)?;
            node = Node::Binary(op, Box::new(node), Box::new(rhs));
        }

        // Boucle ternaire `? :` (prio 0, associatif à droite, marqueur `:`).
        loop {
            let t = self.peek();
            if t.kind != TokKind::Symbol || t.value != "?" {
                break;
            }
            if 0 < level {
                break;
            }
            self.consume();
            let middle = self.parse_level(0)?;
            let t = self.peek();
            if t.kind != TokKind::Symbol || t.value != ":" {
                return Err(CalcError {
                    pos: t.pos,
                    msg: format!("ternaire invalide : `{}`, attendu `:`", t.value),
                });
            }
            self.consume();
            let right = self.parse_level(0)?;
            node = Node::Ternary(Box::new(node), Box::new(middle), Box::new(right));
        }

        Ok(node)
    }

    /// Port de `parseProduction` : parenthèse, unaire ou feuille.
    fn parse_production(&mut self) -> Result<Node, CalcError> {
        let t = self.peek();
        match t.kind {
            TokKind::Symbol if t.value == "(" => {
                self.consume();
                let node = self.parse_level(0)?;
                let t = self.peek();
                if t.kind != TokKind::Symbol || t.value != ")" {
                    // Go : tolère une parenthèse finale non fermée en EOF
                    // (bug historique du calcul Fire Wall) ; sinon erreur.
                    if t.kind == TokKind::Eof {
                        return Ok(node);
                    }
                    return Err(CalcError { pos: t.pos, msg: "parenthèse non fermée".into() });
                }
                self.consume();
                Ok(node)
            }
            TokKind::Symbol if t.value == "+" || t.value == "-" => {
                self.consume();
                let child = self.parse_level(4)?;
                let op = if t.value == "+" { UnOp::Pos } else { UnOp::Neg };
                Ok(Node::Unary(op, Box::new(child)))
            }
            TokKind::Name | TokKind::Number => self.parse_leaf(),
            _ => Err(CalcError {
                pos: t.pos,
                msg: format!("attendu parenthèse, opérateur unaire ou valeur, vu `{}`", t.value),
            }),
        }
    }

    /// Port de `parseLeafCalculation`.
    fn parse_leaf(&mut self) -> Result<Node, CalcError> {
        let t = self.peek();
        if t.kind == TokKind::Number {
            self.consume();
            let v: i64 = t
                .value
                .parse()
                .map_err(|_| CalcError { pos: t.pos, msg: format!("nombre invalide : {}", t.value) })?;
            return Ok(Node::Constant(v));
        }
        if t.kind == TokKind::Name {
            if t.value == "skill" || t.value == "stat" || t.value == "miss" {
                let kind = match t.value.as_str() {
                    "skill" => PropKind::Skill,
                    "stat" => PropKind::Stat,
                    _ => PropKind::Missile,
                };
                return self.parse_property(kind);
            }
            if t.value == "min" || t.value == "max" || t.value == "rand" {
                let f = match t.value.as_str() {
                    "min" => FixedFn::Min,
                    "max" => FixedFn::Max,
                    _ => FixedFn::Rand,
                };
                return self.parse_fixed_fn(f);
            }
            self.consume();
            return Ok(Node::Bare(t.value));
        }
        Err(CalcError {
            pos: t.pos,
            msg: format!("`{}` n'est ni fonction, ni propriété, ni nombre", t.value),
        })
    }

    /// `min(a,b)` / `max(a,b)` / `rand(a,b)` — port de `parseFunction` (le Go
    /// route `rand` sur math/rand ; le port passe par [`EvalContext::random`]).
    fn parse_fixed_fn(&mut self, f: FixedFn) -> Result<Node, CalcError> {
        self.consume(); // nom
        let t = self.peek();
        if t.kind != TokKind::Symbol || t.value != "(" {
            return Err(CalcError { pos: t.pos, msg: "fonction : `(` attendu".into() });
        }
        self.consume();
        let a = self.parse_level(0)?;
        let t = self.peek();
        if t.kind != TokKind::Symbol || t.value != "," {
            return Err(CalcError { pos: t.pos, msg: "fonction : `,` attendu".into() });
        }
        self.consume();
        let b = self.parse_level(0)?;
        let t = self.peek();
        if t.kind != TokKind::Symbol || t.value != ")" {
            return Err(CalcError { pos: t.pos, msg: "fonction : `)` attendu".into() });
        }
        self.consume();
        Ok(Node::FixedFn(f, Box::new(a), Box::new(b)))
    }

    /// `skill('X'.q)` — port de `parseProperty` ; accepte la forme réelle
    /// v1.12 (`.` qualificateur DANS les parenthèses) ET la forme OD2
    /// (`skill('X').q`, `.` hors parenthèses).
    fn parse_property(&mut self, kind: PropKind) -> Result<Node, CalcError> {
        self.consume(); // mot-clé
        let kw = match kind {
            PropKind::Skill => "skill",
            PropKind::Stat => "stat",
            PropKind::Missile => "miss",
        };
        let t = self.peek();
        if t.kind != TokKind::Symbol || t.value != "(" {
            return Err(CalcError {
                pos: t.pos,
                msg: format!("{kw} : parenthèse ouvrante manquante"),
            });
        }
        self.consume();
        let t = self.peek();
        if t.kind != TokKind::Str {
            return Err(CalcError {
                pos: t.pos,
                msg: format!("{kw} : nom entre apostrophes attendu"),
            });
        }
        let name = t.value.clone();
        self.consume();

        let mut qual = String::new();
        let t = self.peek();
        if t.kind == TokKind::Symbol && t.value == "." {
            // Forme v1.12 : skill('X'.qual)
            self.consume();
            let t = self.peek();
            if t.kind != TokKind::Name {
                return Err(CalcError {
                    pos: t.pos,
                    msg: format!("{kw} : qualificateur invalide"),
                });
            }
            qual = t.value.clone();
            self.consume();
            let t = self.peek();
            if t.kind != TokKind::Symbol || t.value != ")" {
                return Err(CalcError {
                    pos: t.pos,
                    msg: format!("{kw} : parenthèse fermante manquante"),
                });
            }
            self.consume();
        } else if t.kind == TokKind::Symbol && t.value == ")" {
            // Forme OD2 : skill('X').qual
            self.consume();
            let t = self.peek();
            if t.kind == TokKind::Symbol && t.value == "." {
                self.consume();
                let t = self.peek();
                if t.kind != TokKind::Name {
                    return Err(CalcError {
                        pos: t.pos,
                        msg: format!("{kw} : qualificateur invalide"),
                    });
                }
                qual = t.value.clone();
                self.consume();
            }
        } else {
            return Err(CalcError {
                pos: t.pos,
                msg: format!("{kw} : `.` ou `)` attendu après le nom"),
            });
        }

        Ok(Node::Prop(kind, name, qual))
    }
}

/// Un CalcString D2 : source d'origine + AST évaluable.
///
/// Parse tolérant : chaîne vide ⇒ [`Node::Constant`] 0 (Go) ; erreur ⇒ l'AST
/// vaut 0 à l'évaluation et [`CalcString::error`] porte la cause — jamais de
/// panic (contrat d2rules), écart assumé vis-à-vis du Go qui `panic!`.
#[derive(Debug, Clone, PartialEq)]
pub struct CalcString {
    src: String,
    ast: Node,
    err: Option<CalcError>,
}

impl CalcString {
    /// Parse une colonne `*Calc` de table (déjà trimmée par le lecteur TSV).
    pub fn parse(src: &str) -> CalcString {
        let trimmed = src.trim();
        if trimmed.is_empty() {
            return CalcString { src: src.into(), ast: Node::Constant(0), err: None };
        }
        let mut parser = Parser { lex: Lexer::new(trimmed) };
        match parser.parse_level(0) {
            Ok(ast) => {
                // Pare-feu fin d'expression : la tolérance Go ne couvre que la
                // parenthèse finale non fermée EN EOF (bug Fire Wall) — tout
                // token résiduel après l'expression est une erreur (sinon un
                // suffixe `)x` serait avalé en silence, éval falsifiée).
                let t = parser.peek();
                if t.kind != TokKind::Eof {
                    return CalcString {
                        src: src.into(),
                        ast: Node::Constant(0),
                        err: Some(CalcError {
                            pos: t.pos,
                            msg: format!("token inattendu après l'expression : `{}`", t.value),
                        }),
                    };
                }
                CalcString { src: src.into(), ast, err: None }
            }
            Err(err) => CalcString { src: src.into(), ast: Node::Constant(0), err: Some(err) },
        }
    }

    /// Source d'origine (brute).
    pub fn source(&self) -> &str {
        &self.src
    }

    /// Erreur de parse, le cas échéant.
    pub fn error(&self) -> Option<&CalcError> {
        self.err.as_ref()
    }

    /// La formule a été parsée sans erreur.
    pub fn is_ok(&self) -> bool {
        self.err.is_none()
    }

    /// Évalue la formule dans `ctx`. Erreur de parse ou vide ⇒ 0.
    pub fn eval(&self, ctx: &mut dyn EvalContext) -> i64 {
        if self.err.is_some() {
            return 0;
        }
        eval_node(&self.ast, ctx)
    }
}

impl fmt::Display for CalcString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.src)
    }
}

fn eval_node(node: &Node, ctx: &mut dyn EvalContext) -> i64 {
    match node {
        Node::Constant(v) => *v,
        Node::Binary(op, a, b) => op.apply(eval_node(a, ctx), eval_node(b, ctx)),
        Node::Unary(op, a) => {
            let v = eval_node(a, ctx);
            match op {
                UnOp::Pos => v,
                UnOp::Neg => v.wrapping_neg(),
            }
        }
        Node::Ternary(cond, then, els) => {
            // Paresseux : D2 n'évalue pas la branche non prise (écart Go,
            // qui évalue les 3 et applique l'op).
            if eval_node(cond, ctx) != 0 {
                eval_node(then, ctx)
            } else {
                eval_node(els, ctx)
            }
        }
        Node::Prop(kind, name, qual) => match kind {
            PropKind::Skill => ctx.skill_level(name, qual),
            PropKind::Stat => ctx.stat_value(name, qual),
            PropKind::Missile => ctx.missile_prop(name, qual),
        },
        Node::Bare(name) => ctx.bare(name),
        Node::FixedFn(f, a, b) => {
            let (va, vb) = (eval_node(a, ctx), eval_node(b, ctx));
            match f {
                FixedFn::Min => va.min(vb),
                FixedFn::Max => va.max(vb),
                FixedFn::Rand => ctx.random(va, vb),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests unitaires (grammaire ; les tests données réelles vivent dans
// parity_tests.rs)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn eval_str(src: &str) -> i64 {
        CalcString::parse(src).eval(&mut NullCtx)
    }

    #[test]
    fn lexer_tokens() {
        let mut lx = Lexer::new("skill('Fire Ball'.blvl)+12>=3");
        let toks = std::iter::from_fn(|| {
            let t = lx.next_token();
            (t.kind != TokKind::Eof).then_some(t)
        })
        .collect::<Vec<_>>();
        let kinds = toks.iter().map(|t| t.kind.clone()).collect::<Vec<_>>();
        assert_eq!(
            kinds,
            vec![
                TokKind::Name,    // skill
                TokKind::Symbol,  // (
                TokKind::Str,     // 'Fire Ball'
                TokKind::Symbol,  // .
                TokKind::Name,    // blvl
                TokKind::Symbol,  // )
                TokKind::Symbol,  // +
                TokKind::Number,  // 12
                TokKind::Symbol,  // >=
                TokKind::Number,  // 3
            ]
        );
        assert_eq!(toks[2].value, "Fire Ball");
        assert_eq!(toks[8].value, ">=");
    }

    #[test]
    fn precedence_and_associativity() {
        assert_eq!(eval_str("1+2*3"), 7);
        assert_eq!(eval_str("(1+2)*3"), 9);
        assert_eq!(eval_str("2^3^2"), 512, "puissance associative à droite");
        assert_eq!(eval_str("7-2-1"), 4, "soustraction associative à gauche");
        assert_eq!(eval_str("2*3+4*5"), 26);
        assert_eq!(eval_str("100/7"), 14, "division tronquée");
        assert_eq!(eval_str("-5/2"), -2, "tronque vers zéro comme Go");
        assert_eq!(eval_str("1+2==3"), 1);
        assert_eq!(eval_str("1+2==4"), 0);
        assert_eq!(eval_str("3!=4"), 1);
        assert_eq!(eval_str("(2>1)+(2<1)"), 1);
        assert_eq!(eval_str("2>=2"), 1);
        assert_eq!(eval_str("1<=0"), 0);
    }

    #[test]
    fn unary_and_ternary() {
        assert_eq!(eval_str("-3+5"), 2);
        assert_eq!(eval_str("+-1"), -1);
        assert_eq!(eval_str("1?10:20"), 10);
        assert_eq!(eval_str("0?10:20"), 20);
        assert_eq!(eval_str("0?1:0?2:3"), 3, "ternaire imbriquée à droite");
        assert_eq!(eval_str("1?2:(0?4:5)"), 2);
    }

    #[test]
    fn fixed_functions() {
        assert_eq!(eval_str("min(3,7)"), 3);
        assert_eq!(eval_str("max(3,7)"), 7);
        assert_eq!(eval_str("min(1+1,2*2)"), 2);
        // rand déterministe : NullCtx renvoie v1.
        assert_eq!(eval_str("rand(11,22)"), 11);
    }

    #[test]
    fn empty_and_errors_never_panic() {
        assert_eq!(CalcString::parse("").eval(&mut NullCtx), 0);
        assert_eq!(CalcString::parse("   ").eval(&mut NullCtx), 0);
        let bad = CalcString::parse("1+");
        assert_eq!(bad.eval(&mut NullCtx), 0, "erreur de parse ⇒ 0");
        assert!(bad.error().is_some());
        assert_eq!(eval_str("1/0"), 0, "division par zéro ⇒ 0");
        assert_eq!(eval_str("0^-1"), 0, "exposant négatif ⇒ 0");
    }

    #[test]
    fn unclosed_paren_tolerance_firewall() {
        // Tolérance Go : `)` manquante en fin de chaîne (bug Fire Wall historique).
        assert_eq!(eval_str("(5+3"), 8);
        // La tolérance vaut aussi pour une expression plus longue non fermée.
        assert_eq!(eval_str("(5+3*2"), 11);
        // Mais un token inattendu après la sous-expression est une erreur.
        assert!(CalcString::parse("(5+3)x").error().is_some());
    }

    /// Contexte de test : skill('X'.blvl) → table, noms nus mappés.
    struct Ctx {
        blvl: Vec<(String, i64)>,
        own_lvl: i64,
    }
    impl EvalContext for Ctx {
        fn skill_level(&mut self, skill: &str, qual: &str) -> i64 {
            let _ = qual;
            self.blvl.iter().find(|(k, _)| k == skill).map(|(_, v)| *v).unwrap_or(0)
        }
        fn bare(&mut self, name: &str) -> i64 {
            match name {
                "lvl" | "sklvl" => self.own_lvl,
                _ => 1,
            }
        }
    }

    #[test]
    fn property_both_forms() {
        let mut ctx = Ctx { blvl: vec![("Fire Ball".into(), 12)], own_lvl: 3 };
        assert_eq!(
            CalcString::parse("skill('Fire Ball'.blvl)*2").eval(&mut ctx),
            24,
            "forme réelle v1.12 : qualificateur dans les parenthèses"
        );
        assert_eq!(
            CalcString::parse("skill('Fire Ball').blvl*2").eval(&mut ctx),
            24,
            "forme OD2 : qualificateur hors parenthèses"
        );
        assert_eq!(
            CalcString::parse("skill('Missing'.blvl)").eval(&mut ctx),
            0,
            "skill inconnu ⇒ 0"
        );
        assert_eq!(CalcString::parse("lvl*5").eval(&mut ctx), 15, "nom nu lvl");
    }

    #[test]
    fn pow_saturation_no_panic() {
        assert_eq!(eval_str("2^10"), 1024);
        assert_eq!(eval_str("999999999^99"), i64::MAX);
    }

    /// Parité opérateur par opérateur avec le Go : chaque opérateur D2 et sa
    /// priorité sont ceux de `opendiablo2/d2common/d2calculation/d2parser/
    /// operations.go:65-174` (binaire 1..=6, `^` droite op.go:165-172),
    /// unaire `+ -` prio 4 (operations.go:29-46), ternaire `?:` prio 0
    /// (operations.go:48-63), fonctions min/max/rand (operations.go:176-197) ;
    /// classes de caractères du lexer identiques (`d2lexer/lexer.go:160`).
    #[test]
    fn parite_operateurs_go() {
        // Comparaisons prio 1 (operations.go:67-132).
        assert_eq!(eval_str("2==2"), 1);
        assert_eq!(eval_str("2!=3"), 1);
        // Relationnels prio 2.
        assert_eq!(eval_str("1<2"), 1);
        assert_eq!(eval_str("2>1"), 1);
        assert_eq!(eval_str("2<=2"), 1);
        assert_eq!(eval_str("2>=3"), 0);
        // Additifs prio 3, multiplicatifs prio 5 (division entière Go).
        assert_eq!(eval_str("10-4+1"), 7);
        assert_eq!(eval_str("7/2*4"), 12, "7/2=3 (tronqué Go) puis ×4");
        // Puissance prio 6, droite : math.Pow Go (operations.go:170).
        assert_eq!(eval_str("2^3^2"), 512);
        assert_eq!(eval_str("3^2"), 9);
        // Unaires prio 4 : le Go parse `-x` en appelant parseLevel(4)
        // (parser.go:176) — l'exposant `^` (prio 6 ≥ 4) est avalé par
        // l'opérande, donc `-2^2` = -(2^2) = -4, comme le Go.
        assert_eq!(eval_str("-2^2"), -4);
        assert_eq!(eval_str("+5"), 5);
        // Ternaire prio 0, droite (parser.go:104-141) : évaluation paresseuse
        // (écart documenté) — même résultat que l'op Go.
        assert_eq!(eval_str("5<4?10:20"), 20);
        assert_eq!(eval_str("1?2:3?4:5"), 2);
        // min/max/rand (operations.go:176-197) ; rand déterministe via ctx.
        assert_eq!(eval_str("min(-3,2)"), -3);
        assert_eq!(eval_str("max(-3,2)"), 2);
        assert_eq!(eval_str("rand(7,9)"), 7);
    }

    /// Formes du lexer Go (`d2lexer/lexer.go:74-99`) : `=`/`!` isolés sont
    /// invalides (le Go panique lexer.go:79 — ici erreur propre, jamais de
    /// panic), `<`/`>` simples restent des symboles unaires valides.
    #[test]
    fn erreurs_lexer_non_paniquantes() {
        assert!(CalcString::parse("1=2").error().is_some(), "`=` isolé : erreur propre");
        assert!(CalcString::parse("1!2").error().is_some(), "`!` isolé : erreur propre");
        assert!(CalcString::parse("skill('X'.blvl").error().is_some());
        assert!(CalcString::parse("min(1,)").error().is_some());
        assert!(CalcString::parse(")").error().is_some());
        assert!(CalcString::parse("?1:2").error().is_some());
        // Évaluation après erreur ⇒ 0 (jamais de panic).
        assert_eq!(eval_str("1=2"), 0);
    }

    /// Binding `stat(...)` complet via [`StatListCtx`] : id numérique, nom du
    /// dictionnaire, charge/quantité v1.12 (`item_charged_skill` id 204,
    /// `quantity` id 70 — patch_itemstatcost.txt), repli noms, inconnu ⇒ 0.
    #[test]
    fn binding_stat_par_statlist_ids() {
        use crate::statlist::{StatLine, StatList as IdList};
        // Le dictionnaire confirme les ids avant tout (données réelles).
        let dict = crate::stats::StatCostTable::global();
        assert_eq!(dict.get("quantity").unwrap().id, 70);
        assert_eq!(dict.get("item_charged_skill").unwrap().id, 204);
        assert_eq!(dict.get("passive_fire_mastery").unwrap().id, 329);

        let mut ids = IdList::new();
        ids.add(70, 42); // quantité
        ids.add(204, 7); // skill chargé (index de skill)
        ids.add(329, 55); // passive_fire_mastery (Hydra)
        let mut names = crate::stats::StatList::new();
        names.add("item_lifesteal", 9); // hors dictionnaire v1.12? repli noms

        let mut ctx = StatListCtx { ids: &ids, names: Some(&names) };
        // Id numérique direct (langage stat.id des saves).
        assert_eq!(CalcString::parse("stat('70'.val)").eval(&mut ctx), 42);
        assert_eq!(CalcString::parse("stat('204'.val)*10").eval(&mut ctx), 70);
        // Nom du dictionnaire → id (quantité d'item jetable).
        assert_eq!(CalcString::parse("stat('quantity'.val)").eval(&mut ctx), 42);
        // Charge de skill : skill('X'.blvl) + stat('item_charged_skill'.val).
        assert_eq!(
            CalcString::parse("stat('item_charged_skill'.val)").eval(&mut ctx),
            7
        );
        // Forme réelle Hydra (patch_skills.txt L64) : qualifier .accr.
        assert_eq!(
            CalcString::parse("stat('passive_fire_mastery'.accr)").eval(&mut ctx),
            55
        );
        // Repli noms (nom absent du dictionnaire), puis inconnu ⇒ 0.
        assert_eq!(CalcString::parse("stat('item_lifesteal'.val)").eval(&mut ctx), 9);
        assert_eq!(CalcString::parse("stat('inconnue'.val)").eval(&mut ctx), 0);
        // Sans repli : inconnu ⇒ 0, jamais de panic.
        let mut ctx2 = StatListCtx { ids: &ids, names: None };
        assert_eq!(CalcString::parse("stat('item_lifesteal'.val)").eval(&mut ctx2), 0);
        // Arithmétique composite : +25 % par point de quantité.
        assert_eq!(
            CalcString::parse("100*(100+stat('quantity'.val))/100").eval(&mut ctx),
            142
        );
        let _ = StatLine::new(0, 0); // réexport silencieux (interface affixes)
    }
}
