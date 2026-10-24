//! Checksum du .d2s (evidence.md §2 — **corrigé §VALIDATION RÉELLE
//! 2026-10-24** sur Javazon.d2s : les 4 octets du champ (0x0C..0x10) sont
//! traités comme des **zéros**, mais la rotation gauche s'applique quand
//! même à chacun — l'ancienne implémentation sautait l'octet entier
//! (rotation comprise) et calculait 0x6096c058 au lieu du 0x7a36d958
//! stocké par le jeu).
//!
//! `sum = ((sum << 1) | (sum >> 31)) + octet` pour chaque octet du fichier
//! entier, le champ checksum lu comme zéro — rotation gauche 1 bit sur u32.

#![allow(dead_code)]

/// Offset du champ checksum dans l'en-tête.
pub const CHECKSUM_OFFSET: usize = 0x0C;

/// Calcule le checksum d'un fichier .d2s complet (champ 0x0C lu comme zéro,
/// rotation appliquée sur ses 4 octets comme sur le reste).
pub fn compute_checksum(data: &[u8]) -> u32 {
    let mut total: u32 = 0;
    for (i, &byte) in data.iter().enumerate() {
        let b = if (CHECKSUM_OFFSET..CHECKSUM_OFFSET + 4).contains(&i) { 0 } else { byte as u32 };
        total = total.rotate_left(1).wrapping_add(b);
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Valeur de contrôle calculée indépendamment en Python (formule
    /// corrigée : zéros dans le champ, rotation appliquée).
    #[test]
    fn golden_small_buffer() {
        // python3 -c "
        //  d = bytes(range(32)) + b'\xAA' * 4 + bytes(range(36, 64))
        //  t = 0
        //  for i, b in enumerate(d):
        //      v = 0 if 12 <= i < 16 else b
        //      t = ((((t << 1) & 0xFFFFFFFF) | (t >> 31)) + v) & 0xFFFFFFFF
        //  print(hex(t))"  ->  0xaf410040
        let mut data = Vec::new();
        data.extend(0u8..32);
        data.extend([0xAA; 4]); // zone checksum neutralisée
        data.extend(36u8..64);
        assert_eq!(compute_checksum(&data), 0xaf41_0040);
    }

    /// Le champ checksum lui-même n'influence pas le résultat.
    #[test]
    fn checksum_field_is_neutral() {
        let mut a = vec![7u8; 64];
        let mut b = a.clone();
        a[0x0C..0x10].copy_from_slice(&[0; 4]);
        b[0x0C..0x10].copy_from_slice(&[0xFF, 0x00, 0x33, 0x77]);
        assert_eq!(compute_checksum(&a), compute_checksum(&b));
    }

    /// Rotation : un octet 0 ajoute uniquement le bit tourné.
    #[test]
    fn rotation_semantics() {
        let data = [0u8; 8];
        assert_eq!(compute_checksum(&data), 0);
        let data = [1u8; 8];
        // 8 rotations-ajouts de 1 : 1,3,7,15,31,63,127,255
        assert_eq!(compute_checksum(&data), 255);
    }
}
