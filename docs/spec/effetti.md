# Catalogo effetti — specifica derivata

> Generato da `tools/gen_spec.py` leggendo la tabella di localizzazione degli
> effetti inclusa nell'app. Il file originale resta fuori dal repository: qui c'è
> solo l'inventario derivato (identificativo tecnico, nome inglese, nome italiano,
> famiglia funzionale).

## Cosa contiene e cosa non contiene

| Contiene | Non contiene |
|---|---|
| 300 elementi: 119 effetti, 109 parametri, 72 enum | formule o codice degli effetti |
| nomi localizzati in 21 lingue (incl. italiano) | i valori o i range dei parametri |
| l'identificativo tecnico (`effect_*`) | shader, LUT, texture o asset |

**Conseguenza pratica:** il catalogo dice *quali* effetti esistono e come si chiamano,
non *come* sono fatti. Il comportamento (formula, intensità, range) va progettato da
zero nel modulo effetti di pizzart, e testato su casi noti (un Lomo non è definibile
per contratto: è una scelta di prodotto nostra).

## Ripartizione per famiglia

| Famiglia | Effetti |
|---|---:|
| Colore | 22 |
| Pellicola / analogico | 20 |
| Stilizzazione artistica | 20 |
| Luce | 9 |
| Altro | 7 |
| Distorsione | 6 |
| Sfocatura / profondità | 6 |
| Effetti di pixel | 6 |
| Bianco e nero / tinta | 5 |
| Ritocco di persona | 5 |
| Curve / toni | 4 |
| Non identificato (numerato) | 4 |
| Nitidezza / dettaglio | 2 |
| Grana / texture | 1 |
| Texture / pattern | 1 |
| Vignettatura / cornice | 1 |
| **Totale** | **119** |

## Effetti

La colonna *implementazione* parte vuota: è lo stato di avanzamento del modulo effetti.

| # | id tecnico | EN | IT | Famiglia | implementato |
|---:|---|---|---|---|:---:|
| 1 | `effect_bathroom1` | Chopped | Bagno | Altro | ☐ |
| 2 | `effect_bathroom2` | Fan | Bagno | Altro | ☐ |
| 3 | `effect_ghost` | Ghost | Effetto fantasma | Altro | ☐ |
| 4 | `effect_grafit` | Graphite | Grafite | Altro | ☐ |
| 5 | `effect_graphglitch` | Glitch | Glitch | Altro | ☐ |
| 6 | `effect_temp` | Temp | Temperatura | Altro | ☐ |
| 7 | `effect_tinyplanet` | Tiny Planet | Piccolo Pianeta | Altro | ☐ |
| 8 | `effect_bwcross` | B&W Cross | B&N incrociato | Bianco e nero / tinta | ☐ |
| 9 | `effect_bwhdr` | B&W HDR | B&N HDR | Bianco e nero / tinta | ☐ |
| 10 | `effect_bwlowcon` | B&W LowCon | Basso B/N | Bianco e nero / tinta | ☐ |
| 11 | `effect_sepia` | Sepia | Seppia | Bianco e nero / tinta | ☐ |
| 12 | `effect_tintsandtemperature` | Tints&Temperature | Tonalità e temperatura | Bianco e nero / tinta | ☐ |
| 13 | `effect_black_and_white` | Black & White | Bianco & Nero | Colore | ☐ |
| 14 | `effect_blackandwhite` | B&W | B/N | Colore | ☐ |
| 15 | `effect_brightness` | Brightness | Luminosità | Colore | ☐ |
| 16 | `effect_color_grid` | Off Grid | Off Grid | Colore | ☐ |
| 17 | `effect_colored_dots` | Spotted | Puntinato | Colore | ☐ |
| 18 | `effect_coloreye` | Eye Color | Colora occhi | Colore | ☐ |
| 19 | `effect_colorgradient` | ColorGradient | Gradiente colore | Colore | ☐ |
| 20 | `effect_colorize` | Colorize | Colora | Colore | ☐ |
| 21 | `effect_colorreplace` | Color Replace | Sostituisci colore | Colore | ☐ |
| 22 | `effect_colors1` | Colors 1 | Colori 1 | Colore | ☐ |
| 23 | `effect_colors2` | Colors 2 | Colori 2 | Colore | ☐ |
| 24 | `effect_colors3` | Colors 3 | Colori 3 | Colore | ☐ |
| 25 | `effect_colors4` | Colors 4 | Colori 4 | Colore | ☐ |
| 26 | `effect_colorsplash` | Color Splash | Spruzzo Colore | Colore | ☐ |
| 27 | `effect_hue` | Hue | Tinta | Colore | ☐ |
| 28 | `effect_popartcolors` | Popart Colors | Colori PopArt | Colore | ☐ |
| 29 | `effect_saturation` | Saturation | Saturazione | Colore | ☐ |
| 30 | `effect_selectiveadjustment` | Selective Adjustment | Regolazione selettiva | Colore | ☐ |
| 31 | `effect_teethwhitener` | Teeth Whiten | Sbiancamento denti | Colore | ☐ |
| 32 | `effect_vibrant` | Vibrant | Vivacità | Colore | ☐ |
| 33 | `effect_warmcolor` | Warm Color | Colore caldo | Colore | ☐ |
| 34 | `effect_watercolor` | Watercolor | Acquerello | Colore | ☐ |
| 35 | `effect_contrast` | Contrast | Contrasto | Curve / toni | ☐ |
| 36 | `effect_halftonedots` | Halftone Dots | Punti Mezzo Tono | Curve / toni | ☐ |
| 37 | `effect_invert` | Invert | Inverti | Curve / toni | ☐ |
| 38 | `effect_negative` | Negative | Negativo | Curve / toni | ☐ |
| 39 | `effect_cylindermirror` | CylinderMirror | Cilindro | Distorsione | ☐ |
| 40 | `effect_mirrors` | Mirror | Specchio | Distorsione | ☐ |
| 41 | `effect_motion` | Motion | Movimento | Distorsione | ☐ |
| 42 | `effect_shear` | Shear | Rifila | Distorsione | ☐ |
| 43 | `effect_swirled` | Swirled | Fatto vorticare | Distorsione | ☐ |
| 44 | `effect_water` | Water | Acqua | Distorsione | ☐ |
| 45 | `effect_contours` | Contours | Contorno | Effetti di pixel | ☐ |
| 46 | `effect_convolution` | Sharpen | Rendere nitido | Effetti di pixel | ☐ |
| 47 | `effect_emboss` | Emboss | Rilievo | Effetti di pixel | ☐ |
| 48 | `effect_pixelize` | Pixelize | Pixellizza | Effetti di pixel | ☐ |
| 49 | `effect_polygon` | Polygon | Polygon | Effetti di pixel | ☐ |
| 50 | `effect_posterize` | Poster | Poster | Effetti di pixel | ☐ |
| 51 | `effect_noisereduction` | Denoise | RiduzioneRumore | Grana / texture | ☐ |
| 52 | `effect_hdr` | HDR | HDR | Luce | ☐ |
| 53 | `effect_hdr_plus` | HDR Plus | HDR Plus | Luce | ☐ |
| 54 | `effect_hdrtwo` | HDR 2 | HDR 2 | Luce | ☐ |
| 55 | `effect_highlights` | Highlights | Punti luce | Luce | ☐ |
| 56 | `effect_lightcross` | Light Cross | Incrociato lieve | Luce | ☐ |
| 57 | `effect_seafoamlightcross` | Seafoam | Schiuma mare | Luce | ☐ |
| 58 | `effect_shadows` | Shadows | Ombre | Luce | ☐ |
| 59 | `effect_sunlesstan` | Suntan | Abbronzatura | Luce | ☐ |
| 60 | `effect_twilight` | Twilight | Crepuscolo | Luce | ☐ |
| 61 | `effect_crisp` | Crisp | Crisp | Nitidezza / dettaglio | ☐ |
| 62 | `effect_sharpendodger` | Dodger | Evasore | Nitidezza / dettaglio | ☐ |
| 63 | `effect_effect13` | B&WHiCon | Alto contrasto | Non identificato (numerato) | ☐ |
| 64 | `effect_effect24` | Retro | Retro | Non identificato (numerato) | ☐ |
| 65 | `effect_effect40` | Sunny | Sole | Non identificato (numerato) | ☐ |
| 66 | `gen_none` | None | Nessuno | Non identificato (numerato) | ☐ |
| 67 | `effect_bleaching` | Gouache | Guazzo | Pellicola / analogico | ☐ |
| 68 | `effect_bworton` | B&W Blur | Sfocatura B/N | Pellicola / analogico | ☐ |
| 69 | `effect_bwvintage` | B&W Vintage | B&N Vintage | Pellicola / analogico | ☐ |
| 70 | `effect_cinerama` | Cinerama | Cinerama | Pellicola / analogico | ☐ |
| 71 | `effect_cross_process1` | Cross Proc 1 | Cross Proc 1 | Pellicola / analogico | ☐ |
| 72 | `effect_cross_processing` | Cross Proc 2 | Proc. Incrociato 2 | Pellicola / analogico | ☐ |
| 73 | `effect_drama` | Drama | Drama | Pellicola / analogico | ☐ |
| 74 | `effect_fattal1` | Fattal 1 | Fattal 1 | Pellicola / analogico | ☐ |
| 75 | `effect_fattal2` | Fattal 2 | Fattal 2 | Pellicola / analogico | ☐ |
| 76 | `effect_film` | Film | Film | Pellicola / analogico | ☐ |
| 77 | `effect_filmbw` | Film B&W | Film B&N | Pellicola / analogico | ☐ |
| 78 | `effect_holga` | Holga 2 | Holga 2 | Pellicola / analogico | ☐ |
| 79 | `effect_holgaart4` | Holga 1 | Holga 1 | Pellicola / analogico | ☐ |
| 80 | `effect_lomo` | Lomo | Lomo | Pellicola / analogico | ☐ |
| 81 | `effect_neoncola` | Neon | Neon | Pellicola / analogico | ☐ |
| 82 | `effect_orton` | Orton | Orton | Pellicola / analogico | ☐ |
| 83 | `effect_pastelperfect` | Pastel | Pastello | Pellicola / analogico | ☐ |
| 84 | `effect_vintage` | Vintage | Vintage | Pellicola / analogico | ☐ |
| 85 | `effect_vintageivory` | Ivory | Avorio Vintage | Pellicola / analogico | ☐ |
| 86 | `effect_warmamber` | Warm Amber | Ambra Calda | Pellicola / analogico | ☐ |
| 87 | `effect_blemishfix` | Blemish Fix | Correzione difetto | Ritocco di persona | ☐ |
| 88 | `effect_facefix` | Face Fix | Correzione volto | Ritocco di persona | ☐ |
| 89 | `effect_fisheye` | Fish Eye | Fisheye | Ritocco di persona | ☐ |
| 90 | `effect_redeye` | Red Eye | Occhi rossi | Ritocco di persona | ☐ |
| 91 | `effect_zoomeye` | Prism | Prisma | Ritocco di persona | ☐ |
| 92 | `effect_blur` | Blur | Sfoca | Sfocatura / profondità | ☐ |
| 93 | `effect_lensblur` | Lens Blur | Sfocatura lenti | Sfocatura / profondità | ☐ |
| 94 | `effect_motionblur` | Motion Blur | Mosso | Sfocatura / profondità | ☐ |
| 95 | `effect_radialblur` | Radial Blur | Sfocatura radiale | Sfocatura / profondità | ☐ |
| 96 | `effect_smartblur` | Smart Blur | Sfocatura smart | Sfocatura / profondità | ☐ |
| 97 | `effect_zoomblur` | Focal Zoom | Zoom focale | Sfocatura / profondità | ☐ |
| 98 | `effect_caricature` | Caricature | Caricatura | Stilizzazione artistica | ☐ |
| 99 | `effect_cartoonizer` | Cartoonizer | Cartoonizer | Stilizzazione artistica | ☐ |
| 100 | `effect_commic` | Comic | Comico | Stilizzazione artistica | ☐ |
| 101 | `effect_oil` | Oil | Olio | Stilizzazione artistica | ☐ |
| 102 | `effect_oilpainting` | OilPainting | Pittura a olio | Stilizzazione artistica | ☐ |
| 103 | `effect_pencil` | Pencil | Matita | Stilizzazione artistica | ☐ |
| 104 | `effect_popart` | Popart | PopArt | Stilizzazione artistica | ☐ |
| 105 | `effect_popart1` | PopArt 1 | PopArt 1 | Stilizzazione artistica | ☐ |
| 106 | `effect_popart2` | PopArt 2 | PopArt 2 | Stilizzazione artistica | ☐ |
| 107 | `effect_sketcher` | Sketcher | Disegnatore | Stilizzazione artistica | ☐ |
| 108 | `effect_sketcher1` | Sketcher 1 | Disegnatore 1 | Stilizzazione artistica | ☐ |
| 109 | `effect_sketcher2` | Sketcher 2 | Disegnatore 2 | Stilizzazione artistica | ☐ |
| 110 | `effect_stenciler1` | Stenciler 1 | Stampinatore 1 | Stilizzazione artistica | ☐ |
| 111 | `effect_stenciler2` | Stenciler 2 | Stampinatore 2 | Stilizzazione artistica | ☐ |
| 112 | `effect_stenciler3` | Stenciler 3 | Stampinatore 3 | Stilizzazione artistica | ☐ |
| 113 | `effect_stenciler4` | Stenciler 4 | Stampinatore 4 | Stilizzazione artistica | ☐ |
| 114 | `effect_stenciler5` | Stenciler 5 | Stampinatore 5 | Stilizzazione artistica | ☐ |
| 115 | `effect_stenciler6` | Stenciler 6 | Stampinatore 6 | Stilizzazione artistica | ☐ |
| 116 | `effect_stenciler7` | Stenciler 7 | Stampinatore 7 | Stilizzazione artistica | ☐ |
| 117 | `effect_stenciler8` | Stenciler 8 | Stampinatore 8 | Stilizzazione artistica | ☐ |
| 118 | `effect_oldpaper` | Old Paper | Carta antica | Texture / pattern | ☐ |
| 119 | `effect_vignette` | Vignette | Vignetta | Vignettatura / cornice | ☐ |

## Parametri riutilizzabili

I parametri sono il vocabolario dei controlli che l'utente si aspetta di trovare
nel pannello di un effetto (slider, selettore): vanno mappati 1:1 nel modulo effetti.

| id tecnico | EN | IT |
|---|---|---|
| `effect_param_activepoint` | Active Point | Punto attivo |
| `effect_param_amount` | Amount | Ampiezza |
| `effect_param_amplitude` | Amplitude | Ampiezza |
| `effect_param_angle` | Angle | Angolo |
| `effect_param_aperture` | Aperture | Apertura |
| `effect_param_artboard` | Style | Stile |
| `effect_param_backgroundwidth` | Background Width | Larghezza dello Sfondo |
| `effect_param_bglightness` | Background Lightness | Luminosità dello Sfondo |
| `effect_param_black_and_white` | Black and White | Bianco & Nero |
| `effect_param_blendmode` | Blendmode | Mod. Fusione |
| `effect_param_blur` | Blur | Sfoca |
| `effect_param_bluramount` | Blur | Sfoca |
| `effect_param_brightness` | Brightness | Luminosità |
| `effect_param_brushmode` | Brush Mode | Modalità Pennello |
| `effect_param_bwfade` | B&W Fade | Dissolvenza B/N |
| `effect_param_bwmode` | B&W Mode | Modo B/N |
| `effect_param_bwmode_blue` | Blue | Blu |
| `effect_param_bwmode_default` | Default | Predefinito |
| `effect_param_bwmode_green` | Green | Verde |
| `effect_param_bwmode_red` | Red | Rosso |
| `effect_param_centerx` | CenterX | CentraX |
| `effect_param_centery` | CenterY | CentraY |
| `effect_param_color` | Color | Colore |
| `effect_param_color1` | Color1 | Colore1 |
| `effect_param_color2` | Color2 | Colore2 |
| `effect_param_contrast` | Contrast | Contrasto |
| `effect_param_count` | Count | Conteggio |
| `effect_param_customtopcolors` | Custom Top Colors | Colori migliori personalizzati |
| `effect_param_denoising` | Denoising | Eliminazione grana |
| `effect_param_density` | Density | Densità |
| `effect_param_details` | Details | Dettagli |
| `effect_param_direction` | Direction | Direzione |
| `effect_param_dissolve` | Dissolve | Dissolvenza |
| `effect_param_distance` | Distance | Distanza |
| `effect_param_fade` | Fade | Dissolvenza |
| `effect_param_fix` | Fix | Correggere |
| `effect_param_gamma` | Gamma | Gamma |
| `effect_param_gapwidth` | Gap | Intervallo |
| `effect_param_hardness` | Hardness | Durezza |
| `effect_param_height` | Height | Altezza |
| `effect_param_highlights` | Highlights | Punti luce |
| `effect_param_horizont` | Shift | Sposta |
| `effect_param_horizontal` | Horizontal | Orizzontale |
| `effect_param_hue` | Hue | Tinta |
| `effect_param_hue1` | Hue1 | Tinta1 |
| `effect_param_hue2` | Hue2 | Tinta2 |
| `effect_param_hue3` | Hue3 | Tinta3 |
| `effect_param_hue4` | Hue4 | Tinta4 |
| `effect_param_inputshift` | Input Shift | Spostamento ingresso |
| `effect_param_intensity` | Intensity | Intensità |
| `effect_param_left` | Left | Sinistra |
| `effect_param_lenght` | Length | Lunghezza |
| `effect_param_length` | Length | Lunghezza |
| `effect_param_lightness` | Lightness | Luminosità |
| `effect_param_line` | Line | Linea |
| `effect_param_lines` | Lines | Righe |
| `effect_param_magic1` | Magic1 | Magico 1 |
| `effect_param_magic2` | Magic2 | Magico 2 |
| `effect_param_mask1` | Mask1 | Maschera 1 |
| `effect_param_mask2` | Mask2 | Maschera 2 |
| `effect_param_maskcolor` | Mask Color | Colore della Maschera |
| `effect_param_maxblur` | MaxBlur | Max sfocatura |
| `effect_param_maxhue` | Max Hue | Max tinta |
| `effect_param_median` | Median | Mediana |
| `effect_param_minhue` | Min Hue | Min tinta |
| `effect_param_mode` | Mode | Modo |
| `effect_param_noise` | Noise | Rumore |
| `effect_param_offset` | Offset | Offset |
| `effect_param_opacity` | Opacity | Opacità |
| `effect_param_outputshift` | Output Shift | Spostamento uscita |
| `effect_param_pattern` | Pattern | Modello |
| `effect_param_phase` | Phase | Fase |
| `effect_param_pointsize` | Point Size | Dimensione Punto |
| `effect_param_position` | Position | Posizione |
| `effect_param_radius` | Radius | Raggio |
| `effect_param_radiuspreview` | Radius Preview | Anteprima raggio |
| `effect_param_replacehue` | Replace Hue | Sostituisci tinta |
| `effect_param_right` | Right | Destra |
| `effect_param_rotate` | Rotate | Ruota |
| `effect_param_rotation` | Rotation | Rotazione |
| `effect_param_roundcorner` | Round Corner | Angolo smussato |
| `effect_param_saturation` | Saturation | Saturazione |
| `effect_param_scale` | Zoom | Ingrandisci |
| `effect_param_scalefactor` | Scale Factor | Fattore di ridimensionamento |
| `effect_param_seed` | Seed | Densità |
| `effect_param_selectedhue` | Selected Hue | Tinta selezionata |
| `effect_param_shadows` | Shadows | Ombre |
| `effect_param_shape` | Shape | Forma |
| `effect_param_size` | Size | Dimensione |
| `effect_param_smooth` | Smooth | Effetto Seta |
| `effect_param_spacing` | Spacing | Spaziatura |
| `effect_param_stretch` | Stretch | Allunga |
| `effect_param_stripwidth` | Width | Larghezza |
| `effect_param_temp` | Temp | Temperatura |
| `effect_param_temperature` | Temperature | Temperat. |
| `effect_param_tilemode` | Style | Stile |
| `effect_param_time` | Time | Tempo |
| `effect_param_tint` | Tint | Tint |
| `effect_param_tollerance` | Tollerance | Tolleranza |
| `effect_param_unsharp` | Unsharp | Riduci nitidezza |
| `effect_param_usecustomcolors` | Use Custom Colors | Use Custom Colors |
| `effect_param_vertical` | Vertical | Verticale |
| `effect_param_vignette` | Vignette | Vignetta |
| `effect_param_wave` | Wave | Onda |
| `effect_param_whiten` | Whiten | Sbiancare |
| `effect_param_width` | Width | Larghezza |
| `effect_param_x` | X | X |
| `effect_param_y` | Y | Y |
| `effect_param_zoom` | Zoom | Ingrandisci |

## Enum (scelte discrete)

| id tecnico | EN | IT |
|---|---|---|
| `effect_enum_activepoint_point0` | point0 | punto0 |
| `effect_enum_activepoint_point1` | point1 | punto1 |
| `effect_enum_activepoint_point2` | point2 | punto2 |
| `effect_enum_activepoint_point3` | point3 | punto3 |
| `effect_enum_activepoint_point4` | point4 | punto4 |
| `effect_enum_blendmode_clear` | Clear | Pulisci |
| `effect_enum_blendmode_color` | Color | Colore |
| `effect_enum_blendmode_colorburn` | Color Burn | Brucia Colore |
| `effect_enum_blendmode_colordodge` | Color Dodge | Schivare colore |
| `effect_enum_blendmode_copy` | Copy | copia |
| `effect_enum_blendmode_darken` | Darken | Scurire |
| `effect_enum_blendmode_destinationatop` | Destination Atop | Destinazione in cima |
| `effect_enum_blendmode_destinationin` | Destination In | Destinazione interna |
| `effect_enum_blendmode_destinationout` | Destination Out | Destinazione esterna |
| `effect_enum_blendmode_destinationover` | Destination Over | Destinazione fuori |
| `effect_enum_blendmode_difference` | Difference | Differenza |
| `effect_enum_blendmode_exclusion` | Exclusion | Esclusione |
| `effect_enum_blendmode_hardlight` | Hard Light | Luce intensa |
| `effect_enum_blendmode_hue` | Hue | Tinta |
| `effect_enum_blendmode_lighten` | Lighten | Schiarire |
| `effect_enum_blendmode_luminosity` | Luminosity | Luminosità |
| `effect_enum_blendmode_multiply` | Multiply | Moltiplica |
| `effect_enum_blendmode_normal` | Normal | Normale |
| `effect_enum_blendmode_overlay` | Overlay | Sovrapponi |
| `effect_enum_blendmode_plusdarker` | Plus Darker | Più scuro plus |
| `effect_enum_blendmode_pluslighte` | Plus Light | Luce plus |
| `effect_enum_blendmode_saturation` | Saturation | Saturazione |
| `effect_enum_blendmode_screen` | Screen | Schermo |
| `effect_enum_blendmode_softlight` | Soft Light | Luce morbida |
| `effect_enum_blendmode_sourceatop` | Source Atop | Sorgente in cima |
| `effect_enum_blendmode_sourcein` | Source In | Sorgente interna |
| `effect_enum_blendmode_sourceout` | Source Out | Source Out |
| `effect_enum_blendmode_xor` | XOR | Xor |
| `effect_enum_flipmode_horizontal` | Horizontal | Orizzontale |
| `effect_enum_flipmode_vertical` | Vertical | Verticale |
| `effect_enum_mode_blue` | B | B |
| `effect_enum_mode_color` | Color | Colore |
| `effect_enum_mode_cube` | Cube | Cubo |
| `effect_enum_mode_default` | Std | Std |
| `effect_enum_mode_exclusion` | Exclusion | Esclusione |
| `effect_enum_mode_grayscale` | Grayscale | Scala di grigi |
| `effect_enum_mode_green` | G | G |
| `effect_enum_mode_l1` | L1 | L1 |
| `effect_enum_mode_l2` | L2 | L2 |
| `effect_enum_mode_l3` | L3 | L3 |
| `effect_enum_mode_l4` | L4 | L4 |
| `effect_enum_mode_l5` | L5 | L5 |
| `effect_enum_mode_linear` | Linear | Lineare |
| `effect_enum_mode_linier` | Linier | Linier |
| `effect_enum_mode_mode1` | Mode1 | Modo1 |
| `effect_enum_mode_mode2` | Mode2 | Modo2 |
| `effect_enum_mode_mode_1` | Mode 1 | Modo1 |
| `effect_enum_mode_mode_2` | Mode 2 | Modo2 |
| `effect_enum_mode_mode_3` | Mode 3 | Modo3 |
| `effect_enum_mode_mode_4` | Mode 4 | Modo4 |
| `effect_enum_mode_multiply` | Multiply | Moltiplica |
| `effect_enum_mode_normal` | Normal | Normale |
| `effect_enum_mode_overlay` | Overlay | Sovrapponi |
| `effect_enum_mode_red` | R | R |
| `effect_enum_mode_screen` | Screen | Schermo |
| `effect_enum_mode_sepia` | Sepia | Seppia |
| `effect_enum_mode_sin` | Sin | Sin |
| `effect_enum_mode_sqrt` | Sqrt | Radice quadrata |
| `effect_enum_mode_square` | Square | Quadrato |
| `effect_enum_shape_circules` | Circules | Cerchi |
| `effect_enum_shape_crosses` | Crosses | Croci |
| `effect_enum_shape_lines` | Lines | Righe |
| `effect_enum_shape_octagon` | Octagon | Ottagono |
| `effect_enum_shape_random` | Random | Random |
| `effect_enum_shape_rhombus` | Rhombus | Rombo |
| `effect_enum_shape_squares` | Squares | Squares |
| `effect_enum_shape_triangle` | Triangle | Triangolo |

## Lingue disponibili nel catalogo

`ar`, `bn`, `de`, `en`, `es`, `fa`, `fr`, `hi`, `hy`, `in`, `it`, `ja`, `ko`, `pl`, `pt`, `ru`, `th`, `tr`, `vi`, `zh`, `zh-HK`

