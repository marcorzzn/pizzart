# Specifiche derivate — solo aggregati

Questi numeri vengono dall'analisi dell'app di riferimento, ma nel repository
entra **solo ciò che abbiamo prodotto noi**: conteggi e classificazioni, non il
materiale originale.

| Cosa | Quanto |
|---|---|
| Schermate dichiarate dal manifest | **243** activity |
| Ingressi da deeplink | **13** |
| Aree funzionali individuate | **9** |
| Effetti e parametri nel catalogo | **119** effetti, **109** parametri, **72** enum |
| Lingue coperte dal catalogo | **21** |
| Termini del vocabolario UI | **79**, di cui **57** osservati nell'app |

## Cosa non c'è, e perché

Il dettaglio riga per riga — identificativi tecnici degli effetti, nomi
localizzati, nomi delle classi Android — è un **dump** dei dati originali: non è
una specifica nostra, è la copia di una tabella di qualcun altro. Resta fuori dal
repository, in `_estrazione/spec_archivio/`, insieme al materiale grezzo da cui è
tratto.

Nel repository restano i **fatti** che abbiamo ricavato (quanti effetti, quante
famiglie, che struttura hanno le schermate) perché sono ciò che serve per
progettare un editor, e sono nostri.

## Rigenerazione

```bash
python tools/estrai_input.py     # copia gli input in ../_estrazione (fuori dal repo)
python tools/gen_spec.py         # riscrive il dettaglio in ../_estrazione/spec_archivio/
```

Il generatore scrive **fuori** dal repository per costruzione: se un giorno
servirà un conteggio nuovo, si aggiorna questa tabella a mano, dopo averlo
verificato. Una specifica che si aggiorna da sola da una fonte esterna è un
modo elegante per reintrodurre materiale di terzi senza accorgersene.

## Perché questa distinzione conta

Un capitolato funzionale ("l'utente si aspetta una sfocatura gaussiana con
raggio e soglia") è un'idea, e le idee non sono soggette a copyright. Un elenco
di 300 identificativi con le rispettive traduzioni è una collezione di dati
altrui. La differenza non è formale: la prima la usi per progettare, la seconda
la stai ridistribuendo.

## Un dettaglio dichiarato

`tools/gen_spec.py` contiene **una** stringa dell'app di riferimento:
il prefisso `effect_param_`, usato per decidere se una voce del catalogo è un
parametro o un effetto. Non è un dato estratto ma una regola di classificazione,
e non rivela nulla che non sia visibile usando l'app. È l'unica occorrenza:
verificata con una scansione di tutti i file versionati (`com.picsart.`,
`effect_*`).