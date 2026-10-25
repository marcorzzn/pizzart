//! Errori delle operazioni sui layer.
//!
//! Le operazioni che possono non avere senso (fondere l'ultimo layer, raggruppare
//! fratelli di gruppi diversi) ritornano un errore invece di fare qualcosa di
//! inatteso: un comando che non si può applicare non deve entrare nella storia.

/// Errori delle operazioni sui layer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LayerError {
    /// Non c'è un layer sotto quello indicato, nello stesso contenitore.
    CannotMergeDown,
    /// I nodi indicati non sono fratelli (non stanno nello stesso contenitore).
    NotSiblings,
    /// La destinazione di uno spostamento non è valida.
    InvalidTarget,
    /// Non è stato indicato alcun nodo.
    EmptySelection,
    /// L'identificativo non esiste nell'albero.
    NotFound,
    /// Il nodo non è del tipo richiesto (per esempio: non è un gruppo).
    NotAGroup,
}

impl std::fmt::Display for LayerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let testo = match self {
            LayerError::CannotMergeDown => "nessun layer sotto da fondere",
            LayerError::NotSiblings => "i nodi non sono fratelli",
            LayerError::InvalidTarget => "destinazione non valida",
            LayerError::EmptySelection => "nessun nodo selezionato",
            LayerError::NotFound => "nodo non trovato",
            LayerError::NotAGroup => "il nodo non è un gruppo",
        };
        f.write_str(testo)
    }
}

impl std::error::Error for LayerError {}
