# Specifiche derivate

Questi documenti **non sono scritti a mano**: li produce `tools/gen_spec.py`
leggendo gli artefatti dell'analisi dell'app di riferimento. Rigenerarli è un
comando, e ogni numero nel documento è verificabile nel file JSON accanto.

| File | Contenuto | Generato da |
|---|---|---|
| `schermate.md` / `.json` | 243 activity, 13 deeplink, aree funzionali, mappa delle rotte | `AndroidManifest.xml` + `apktool.yml` |
| `effetti.md` / `.json` | 119 effetti, 109 parametri, 72 enum, nomi in 21 lingue | tabella di localizzazione degli effetti |
| `glossario.md` | vocabolario UI minimo: 79 termini, 57 osservati nell'app | tabella delle stringhe |

## Rigenerazione

```bash
python tools/estrai_input.py     # copia i 3 input in ../_estrazione (fuori dal repo)
python tools/gen_spec.py         # riscrive docs/spec/
```

`../_estrazione/` contiene il materiale grezzo e non va versionata: gli input si
estrarre dagli archivi ogni volta che servono. Il motivo è semplice: nel
repository entra solo ciò che abbiamo prodotto noi.

## Come leggerli

- `schermate.md` → quali schermate servono e come sono raggruppate. Una riga =
  una schermata che l'utente si aspetta di trovare.
- `effetti.md` → quali effetti esistono e come si chiamano. La colonna
  *implementato* è lo stato di avanzamento del modulo effetti.
- `glossario.md` → come chiamare le cose nell'interfaccia, per non costringere
  nessuno a imparare una nuova terminologia.

## Limiti dichiarati

Sono documenti onesti, con i loro buchi scritti dentro:

- Le **famiglie degli effetti** sono euristiche sul nome tecnico: raggruppano per
  lavoro simile, non pretendono di indovinare le formule. Il catalogo dice
  *quali* effetti esistono, non *come* sono fatti.
- L'**italiano è una scelta di prodotto**: l'app analizzata non include la locale
  italiana in questa build (39 cartelle `values*`, nessuna `values-it`), quindi
  il confronto EN↔IT è possibile solo come terminologia di categoria.
- Le **aree funzionali** derivano dal nome del package con regole deterministiche
  (`AREE` in `gen_spec.py`): alcune schermate restano in "Altro" e sono elencate
  come tali invece di essere forzate.
- Il **perimetro esclude** pubblicità, abbonamento, social, store e mini-app.
  È un'assunzione di prodotto, una riga di codice, e si può ribaltare.
