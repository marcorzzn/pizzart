//! Identificativo stabile di un nodo, pensato per finire nel file.
//!
//! `LayerId` è un contatore: veloce in memoria, inutile su disco (due sessioni
//! numerano diversamente, un copia-incolla collide). Il file ha bisogno di un
//! identificativo **stabile e unico** tra sessioni e documenti.
//!
//! Qui c'è il minimo che serve, **senza dipendenze esterne**: 16 byte casuali
//! con la forma di un UUID v4 (bit di versione e variante a posto), formattabili
//! e leggibili come UUID standard. Il generatore è uno xorshift seminato da
//! tempo, contatore e indirizzo di uno statico: non è crittografico, ma il
//! requisito è l'unicità pratica, non la resistenza ad attacchi.

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Identificativo stabile: 16 byte, forma UUID v4.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StableId([u8; 16]);

impl StableId {
    /// Genera un identificativo nuovo.
    ///
    /// Mescola tempo (nanosecondi), un contatore monotono e l'indirizzo di uno
    /// statico: due documenti creati nello stesso istante su macchine diverse
    /// non possono coincidere per caso.
    pub fn new() -> Self {
        static CONTATORE: AtomicU64 = AtomicU64::new(0);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        let contatore = CONTATORE.fetch_add(1, Ordering::Relaxed);
        let indirizzo = &CONTATORE as *const AtomicU64 as u64;

        let mut seme =
            nanos ^ (contatore.wrapping_mul(0x9E37_79B9_7F4A_7C15)) ^ indirizzo.rotate_left(17);
        if seme == 0 {
            seme = 0xDEAD_BEEF_CAFE_BABE;
        }

        let mut byte = [0u8; 16];
        for pezzo in byte.chunks_mut(8) {
            seme = xorshift64star(seme);
            pezzo.copy_from_slice(&seme.to_le_bytes());
        }
        let mut id = StableId(byte);
        // versione 4 e variante RFC 4122: così è un UUID valido anche per gli altri
        id.0[6] = (id.0[6] & 0x0F) | 0x40;
        id.0[8] = (id.0[8] & 0x3F) | 0x80;
        id
    }

    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }

    /// Forma canonica `xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx`.
    pub fn to_uuid_string(self) -> String {
        let b = self.0;
        format!(
            "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7], b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]
        )
    }

    /// Legge la forma canonica (accetta anche senza trattini).
    pub fn parse_uuid_string(s: &str) -> Option<Self> {
        let cifre: Vec<u8> = s.bytes().filter(|c| *c != b'-').collect();
        if cifre.len() != 32 {
            return None;
        }
        let mut byte = [0u8; 16];
        for i in 0..16 {
            let alto = nibble(cifre[i * 2])?;
            let basso = nibble(cifre[i * 2 + 1])?;
            byte[i] = (alto << 4) | basso;
        }
        Some(StableId(byte))
    }
}

impl Default for StableId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for StableId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_uuid_string())
    }
}

/// Generatore xorshift64*: veloce, deterministico dato il seme, sufficiente per
/// evitare collisioni pratiche.
fn xorshift64star(mut x: u64) -> u64 {
    x ^= x >> 12;
    x ^= x << 25;
    x ^= x >> 27;
    x.wrapping_mul(0x2545_F491_4F6C_DD1D)
}

fn nibble(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn due_id_sono_diversi() {
        let ids: Vec<StableId> = (0..1000).map(|_| StableId::new()).collect();
        let mut ordinati = ids.clone();
        ordinati.sort();
        ordinati.dedup();
        assert_eq!(
            ordinati.len(),
            ids.len(),
            "1000 id devono essere tutti diversi"
        );
    }

    #[test]
    fn la_forma_e_un_uuid_v4_valido() {
        let s = StableId::new().to_uuid_string();
        assert_eq!(s.len(), 36);
        let byte = s.as_bytes();
        assert_eq!(byte[8], b'-');
        assert_eq!(byte[13], b'-');
        assert_eq!(byte[18], b'-');
        assert_eq!(byte[23], b'-');
        assert_eq!(byte[14], b'4', "versione 4");
        assert!(
            matches!(byte[19], b'8' | b'9' | b'a' | b'b'),
            "variante RFC 4122"
        );
    }

    #[test]
    fn testo_e_byte_si_ricostruiscono_a_vicenda() {
        let id = StableId::new();
        let s = id.to_uuid_string();
        assert_eq!(StableId::parse_uuid_string(&s), Some(id));
        // accetta anche senza trattini
        let senza: String = s.chars().filter(|c| *c != '-').collect();
        assert_eq!(StableId::parse_uuid_string(&senza), Some(id));
    }

    #[test]
    fn stringhe_invalide_sono_rifiutate() {
        assert_eq!(StableId::parse_uuid_string(""), None);
        assert_eq!(StableId::parse_uuid_string("non-e-un-uuid"), None);
        assert_eq!(
            StableId::parse_uuid_string("550e8400-e29b-41d4-a716-44665544000z"),
            None,
            "carattere non esadecimale"
        );
    }
}
