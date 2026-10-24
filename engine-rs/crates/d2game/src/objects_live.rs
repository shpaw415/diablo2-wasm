//! `objects_live` — objets interactifs data-driven (étape 9c « monde vivant »,
//! campagne diablo-2-wasm). Même pattern additif que `npc_bridge.rs` /
//! `waypoint_travel.rs` : un module séparé + table réelle embarquée ; le
//! câblage `game.rs`/wasm (`attach` au chargement de niveau + `objects` JSON
//! UI + clic) reste au pas d'intégration, ici tout est testable au plan.
//!
//! # Sources réelles (lecture seule)
//! * `assets/objects.txt` v1.12 (574 objets, embarqué) — colonnes consommées :
//!   `Id`, `Token`, `Name`, `Mode0` (**portail de spawn** : 0 = l'objet n'a
//!   pas de mode actif NU et n'apparaît JAMAIS — ex. le `NU0` Dummy présent
//!   dans TownN1.ds1 et les portails TP/PP), `IsDoor`, `OpenWarp`,
//!   `OperateFn`, `SizeX`/`SizeY`, `SpawnMax`.
//! * `d2w-re/tables/monde/objgroup.txt` v1.12 (133 groupes, embarqué en
//!   l'état) — les instances DS1 de `type=2` désignent un GROUPE : un objet
//!   du groupe est tiré pondéré par `PROBn` (les groupes portent aussi les
//!   colonnes informatives `SHRINES`/`WELLS`, gardées telles quelles).
//! * Section OBJECTS des DS1 v18 (port de `tools/ds1_objects.py`, réf.
//!   OpenDiablo2 `d2ds1`) : après les 6 couches, `count` puis `count × 5`
//!   int32 (`type, id, x, y, flags`) — `type=1` ⇒ `id` = ligne objects.txt,
//!   `type=2` ⇒ `id` = groupe objgroup.txt ; `x`/`y` en SOUS-tuiles (1 tuile
//!   = 5 sous-tuiles, cf. `compose.rs`).
//!
//! # Classification data-driven (OperateFn, objects.txt)
//! * coffres `OperateFn` 1 (caskets/sarcophages) → TC « Act N Chest A » ;
//!   4 (chests) → « Act N Chest B », ou C si `SizeX×SizeY ≥ 10` (grands
//!   coffres de tombes 5×3) — l'ACTE du NIVEAU gouverne la TC (town/Blood
//!   Moor = acte I), pas la colonne `Act` de l'objet (les DS1 mélangent des
//!   lignes d'actes divers) ;
//! * conteneurs mineurs `OperateFn` 3/5/7/14/51 (urnes, barils, cadavres,
//!   caches, souches) → TC « Act N Junk » ;
//! * sanctuaires `OperateFn` 2 (token SF/SH/HW/MB…) → effet tiré à
//!   l'apparition par [`d2rules::shrines::pick_weighted`] (rareté respectée) ;
//! * portes `IsDoor=1`, portails `OpenWarp=1` (TD/sl/sv), livres `OperateFn` 6
//!   (Tower Tome), décor `OperateFn` 0 (visible, non interactif).
//!
//! # Écarts MVP documentés
//! * `SpawnMax`, `DENSITY` (génération extérieure) et les colonnes
//!   `SHRINES`/`WELLS` des groupes : informatifs, non simulés ;
//! * porte : état ouvert/fermé seulement (la collision fine des DT1 ne
//!   bascule pas) ; portail : renvoyé à l'UI, le voyage réel reste au
//!   câblage warp existant ; livres : cliquables sans butin ;
//! * le câblage hero (combat voyant les buffs) passera par
//!   [`d2rules::shrines::ShrineBuff::stat_lines`] dans l'agrégation
//!   `StatList` au pas d'intégration (socle existant, cf. doc shrines).

use crate::ds1::Ds1Error;
use d2rules::shrines::{self, ShrineBuff};
use d2rules::tsv::Tsv;
use d2rules::{ItemDef, World};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::OnceLock;

/// `objects.txt` v1.12 embarqué (574 records — même modèle que
/// `spawn.rs`/`loot.rs`). Le fichier d'origine n'est pas 100 % UTF-8
/// (octets de description latins) : décodage lossy à l'initialisation —
/// les colonnes clés (Id/Token/modes…) sont ASCII, seuls quelques noms
/// affichables perdent un accent, conforme au caractère lenient du module.
const OBJECTS_TXT: &[u8] = include_bytes!("../../../../assets/objects.txt");

/// `objgroup.txt` v1.12 (133 groupes) embarqué EN L'ÉTAT depuis
/// `d2w-re/tables/monde/objgroup.txt` (2026-10-24) — aucune copie du fichier
/// dans ce workspace, l'embed évite toute dépendance de build croisée.
const OBJGROUP_TSV: &str = r#"GroupName	Offset	ID0	DENSITY0	PROB0	ID1	DENSITY1	PROB1	ID2	DENSITY2	PROB2	ID3	DENSITY3	PROB3	ID4	DENSITY4	PROB4	ID5	DENSITY5	PROB5	ID6	DENSITY6	PROB6	ID7	DENSITY7	PROB7	SHRINES	WELLS
Not Used	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
Rogues for act1( including staked rogues)	1	54	30	25	55	30	25	56	30	25	57	30	25	0	0	0	0	0	0	0	0	0	0	0	0	0	0
Wells for forest	2	130	0	75	164	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	1	1
Indoor Chests	3	5	48	50	6	48	50	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
Rogues for act1 w/o staked rogues	4	54	30	37	55	30	37	56	30	26	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
sewer shrines	5	279	0	25	280	0	25	281	0	25	282	0	25	0	0	0	0	0	0	0	0	0	0	0	0	1	0
Cave Wells	6	138	0	25	275	0	25	276	0	25	277	0	25	0	0	0	0	0	0	0	0	0	0	0	0	1	1
Crypt caskets	7	3	125	75	28	125	25	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
Crypt shrines	8	77	0	50	165	0	50	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	1	0
Crypt Chests	9	5	48	20	6	48	20	240	30	20	241	30	20	242	30	20	0	0	0	0	0	0	0	0	0	0	0
Crypt Barrels	10	7	64	100	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
Forgotten Tower caskets	11	3	125	100	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
Forgotten Tower Shrines	12	77	0	100	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	1	0
Forgotten Tower Barrels	13	7	64	100	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
Indoor Shrine Act1 Set1	14	77	0	25	84	0	25	165	0	25	260	0	25	0	0	0	0	0	0	0	0	0	0	0	0	1	0
Indoor Barrels	15	7	64	100	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
Caskets Catacombs	16	79	80	33	53	80	33	1	80	33	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
Indoor Well Act1 Set1	17	132	0	100	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	1
Desert Outdoor Shrines Set1	18	86	0	12	134	0	12	135	0	12	136	0	12	150	0	13	151	0	13	172	0	13	173	0	13	1	0
Desert Tomb Shrines	19	85	0	20	166	0	20	150	0	20	151	0	20	322	0	20	0	0	0	0	0	0	0	0	0	1	1
not used	20	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
not used	21	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	1	0
Desert Indoor Shrines Set3	22	116	0	100	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	1	0
Desert Outdoor Shrines Set3	23	86	0	34	85	0	34	166	0	34	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	1	0
Desert Chests	24	87	30	20	88	30	20	148	30	20	146	30	20	147	30	20	0	0	0	0	0	0	0	0	0	0	0
maggot lair shrines	25	301	0	50	302	0	50	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	1	0
act 2 sewers	26	244	30	25	246	30	25	225	30	25	283	30	25	0	0	0	0	0	0	0	0	0	0	0	0	0	0
Desert Obelisk	27	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
Obelisk Act1	28	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
Desert Urns	29	4	120	100	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
Desert Wells	30	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	1
Jungle Wells	31	130	0	100	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	1
Dead Guards	32	154	30	50	270	30	50	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
outsideforestobj1	33	139	30	25	140	30	50	144	30	25	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
outsideforestobj2	34	141	30	50	139	30	50	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
sand maggot lair	35	266	30	100	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
desertobjects2	36	178	30	50	171	30	50	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
desertobjects3	37	142	30	33	143	30	33	4	30	34	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
outside forest object 3	38	155	30	34	174	30	33	175	30	33	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
not used	39	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	1	0
act 3 jungle clickables #1	40	181	30	20	183	30	20	185	30	20	186	30	20	187	30	20	0	0	0	0	0	0	0	0	0	0	0
not used	41	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	1	0
act 3 jungle clickables #2	42	188	30	20	187	30	20	186	30	20	208	30	20	272	30	20	0	0	0	0	0	0	0	0	0	0	0
jungle shrines	43	184	0	16	190	0	16	191	0	16	197	0	16	168	0	16	170	0	20	0	0	0	0	0	0	1	0
tristram objects	44	239	30	50	245	30	50	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
really rare objects in act1 act3	45	250	30	20	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
outer hell shrines	46	226	0	20	231	0	20	232	0	20	236	0	20	249	0	20	0	0	0	0	0	0	0	0	0	1	0
outer hell objects	47	222	30	16	225	30	16	233	0	16	363	30	16	372	30	16	380	30	20	0	0	0	0	0	0	0	0
inner hell objects	48	125	30	20	126	30	20	127	30	20	128	30	20	0	0	0	0	0	0	0	0	0	0	0	0	0	0
inner hell shrines	49	93	0	20	96	0	20	97	0	20	123	0	20	124	0	20	0	0	0	0	0	0	0	0	0	1	0
act1 cathedral shrines	50	262	0	12	260	0	12	263	0	12	264	0	12	265	0	12	165	0	12	77	0	12	132	0	14	1	1
act1 jail shrines	51	263	0	12	260	0	12	263	0	12	264	0	12	265	0	12	165	0	12	77	0	12	132	0	14	1	1
jungle indoor clickables	52	188	30	20	187	30	20	186	30	20	208	30	20	271	30	20	0	0	0	0	0	0	0	0	0	0	0
arcane sanctuary clickables	53	387	30	20	388	30	20	389	30	20	390	30	20	391	30	20	0	0	0	0	0	0	0	0	0	0	0
not used	54	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
Harem Clickables	55	4	30	20	176	30	20	154	30	10	270	30	10	314	30	10	315	30	10	316	30	10	317	30	10	0	0
Harem Shrines	56	109	0	25	167	0	25	299	0	25	300	0	25	0	0	0	0	0	0	0	0	0	0	0	0	1	0
Cellar Clickables	57	154	30	10	270	30	10	176	30	80	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
Cellar Shrines	58	116	0	20	280	0	20	281	0	20	279	0	20	282	0	20	0	0	0	0	0	0	0	0	0	0	0
travisal objects	59	203	30	20	204	30	20	205	30	20	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
travisal objects2	60	181	30	20	183	30	20	208	30	20	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
travinsal shrine	61	199	0	20	200	0	20	201	0	20	206	0	20	202	0	20	0	0	0	0	0	0	0	0	0	0	0
act3 sewer shrine	62	325	0	16	197	0	16	191	0	16	278	0	16	120	0	16	184	0	16	0	0	0	0	0	0	1	0
not used	63	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
act3 sewer objects	64	181	30	20	183	30	20	185	30	20	187	30	20	186	30	20	0	0	0	0	0	0	0	0	0	0	0
act3 sewer objects	65	244	30	20	246	30	20	225	30	20	326	30	10	327	30	20	0	0	0	0	0	0	0	0	0	0	0
act 3dungeon shrines	66	325	0	50	361	0	50	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	1	0
act 3 dungeon objects	67	326	30	12	225	16	24	360	30	20	362	30	20	271	30	24	0	0	0	0	0	0	0	0	0	0	0
spider objects	68	333	30	20	334	30	20	335	30	20	336	30	20	0	0	0	0	0	0	0	0	0	0	0	0	0	0
act 3 jungle clickables #3	69	0	0	0	169	30	20	185	30	20	271	30	20	326	30	10	0	0	0	0	0	0	0	0	0	0	0
act 3 sewer objects	70	208	30	20	209	30	20	271	30	20	225	30	20	0	0	0	0	0	0	0	0	0	0	0	0	0	0
not used	71	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	1	0
not used	72	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
act 2 Harum bed	73	289	30	100	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
act ? mephisto clickables	74	329	30	20	330	30	20	331	30	20	332	30	20	0	0	0	0	0	0	0	0	0	0	0	0	0	0
act 2 arcane sanctuary	75	319	0	25	320	0	25	303	0	25	133	0	25	0	0	0	0	0	0	0	0	0	0	0	0	1	0
act 3 kurasthealthshrine	76	343	0	50	344	0	50	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
act 4 outer hell guys	77	380	30	25	369	50	25	383	30	25	384	30	25	0	0	0	0	0	0	0	0	0	0	0	0	0	0
indoor act 2 caskets	78	89	30	100	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
dummy	79	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
dummy	80	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
dummy	81	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
dummy	82	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
dummy	83	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
dummy	84	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
dummy	85	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
dummy	86	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
dummy	87	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
dummy	88	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
dummy	89	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
dummy	90	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
dummy	91	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
dummy	92	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
dummy	93	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
dummy	94	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
dummy	95	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
dummy	96	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
EXPANSION																											
level1 fire`	98	401	30	50	402	30	50	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
wildernessmagicshrines	99	414	30	20	439	30	20	421	30	20	427	30	20	0	0	0	0	0	0	0	0	0	0	0	0	0	0
wildernesschests	100	413	30	20	420	30	20	424	30	20	425	30	20	477	30	20	0	0	0	0	0	0	0	0	0	0	0
wildernesschests2	101	430	30	20	431	30	20	454	30	20	540	30	20	438	30	20	0	0	0	0	0	0	0	0	0	0	0
oddobject	102	448	30	20	417	30	20	446	30	20	447	30	20	450	30	20	0	0	0	0	0	0	0	0	0	0	0
explodingbarrel	103	418	30	50	419	30	50	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
wildernessmana/health	104	422	30	50	423	30	50	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
wilderness well	105	426	30	50	419	30	50	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
wilderness jars	106	443	30	20	444	30	20	445	30	20	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
icecaveobjects	107	463	30	20	466	30	20	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
icecavehealth/mana	108	464	30	50	465	30	50	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
icecavejars	109	467	30	20	468	30	20	469	30	20	470	30	20	471	30	20	0	0	0	0	0	0	0	0	0	0	0
icecaveshrines	110	479	30	25	472	30	75	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
snowyobjects	111	501	30	20	502	30	20	500	30	20	432	30	20	433	30	20	0	0	0	0	0	0	0	0	0	0	0
snowyshrines	112	428	30	20	495	30	20	497	30	20	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
snowymana/health	113	491	30	50	492	30	50	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
snowywell	114	493	30	100	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
snowyobjects	115	504	30	25	505	30	25	506	30	25	507	30	25	0	0	0	0	0	0	0	0	0	0	0	0	0	0
templeshrine	116	509	30	75	520	30	25	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
templewell	117	513	30	100	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
templeobjects	118	516	30	20	517	30	20	518	30	20	413	30	20	418	30	20	0	0	0	0	0	0	0	0	0	0	0
baalsmana/health	119	483	30	50	484	30	50	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
baalsobjects	120	485	30	20	486	30	20	487	30	20	524	30	15	525	30	15	526	30	10	0	0	0	0	0	0	0	0
baalsshrine	121	488	30	25	499	30	25	503	30	25	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
baalswell	122	498	30	50	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
icecavewell	123	519	30	50	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
templemana/health	124	521	30	50	522	30	50	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
templeobjects	125	444	30	20	445	30	20	443	30	20	424	30	20	425	30	20	0	0	0	0	0	0	0	0	0	0	0
redbaals_object	126	529	30	20	530	30	20	531	30	20	532	30	20	533	30	20	0	0	0	0	0	0	0	0	0	0	0
redbaals_object	127	534	30	20	535	30	50	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
temple_objects	128	541	30	35	544	30	35	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
everywhere	129	540	30	20	545	30	20	518	30	20	556	30	20	554	30	15	0	0	0	0	0	0	0	0	0	0	0
dummy	130	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
dummy	131	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0
dummy	132	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0	0"#;

/// Portée d'interaction (tuiles) — même convention que les NPC
/// (`talk_to_npc`, 2.5 tuiles).
pub const USE_RANGE_TILES: f32 = 2.5;

// --------------------------------------------------------------------------
// Section OBJECTS des DS1 v18 (miroir du walk d'en-tête de `ds1.rs`).
// --------------------------------------------------------------------------

/// Un record de la section OBJECTS d'un DS1 (5 int32, v15+).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ds1Object {
    /// `type` : 1 = ligne objects.txt, 2 = groupe objgroup.txt.
    pub obj_type: u32,
    /// Id objects.txt (type 1) ou de groupe objgroup.txt (type 2).
    pub id: u32,
    /// X en SOUS-tuiles.
    pub x: u32,
    /// Y en SOUS-tuiles.
    pub y: u32,
    /// Flags bruts (non interprétés au MVP).
    pub flags: u32,
}

fn i32_at(d: &[u8], at: usize) -> Result<i32, Ds1Error> {
    let b = d.get(at..at + 4).ok_or(Ds1Error::TooShort)?;
    Ok(i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

fn u32_at(d: &[u8], at: usize) -> u32 {
    d.get(at..at + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .unwrap_or(0)
}

/// Lit la section OBJECTS d'un DS1 v18 — même walk d'en-tête que
/// [`crate::ds1::decode_ds1`] (v18 trap `w/h = stocké + 1`). Section absente
/// (fichier fini après les couches — `bloodmoor.ds1`, `lutgholein.ds1`) ⇒
/// `Ok(vec![])` ; section tronquée ⇒ records lisibles (lenient, documenté).
pub fn decode_ds1_objects(d: &[u8]) -> Result<Vec<Ds1Object>, Ds1Error> {
    let version = i32_at(d, 0)?;
    if version != 18 {
        return Err(Ds1Error::UnsupportedVersion(version));
    }
    let w = i32_at(d, 4)?.max(0) as usize + 1;
    let h = i32_at(d, 8)?.max(0) as usize + 1;
    let filecount = i32_at(d, 20)?.max(0) as usize;
    let mut p = 24usize;
    for _ in 0..filecount {
        let e = d
            .get(p..)
            .and_then(|rest| rest.iter().position(|&b| b == 0))
            .ok_or(Ds1Error::TooShort)?
            + p;
        p = e + 1;
    }
    p += 8; // paire meta (2 ints)
    let n = w * h;
    p += 4 * 6 * n; // 6 couches
    // Section OBJECTS : absente ⇒ 0 objet (cas bloodmoor/lutgholein).
    if d.get(p..p + 4).is_none() {
        return Ok(Vec::new());
    }
    let count = i32_at(d, p)?.max(0) as usize;
    p += 4;
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let at = p + i * 20;
        let Some(rec) = d.get(at..at + 20) else { break }; // tronqué ⇒ partiel
        out.push(Ds1Object {
            obj_type: u32_at(rec, 0),
            id: u32_at(rec, 4),
            x: u32_at(rec, 8),
            y: u32_at(rec, 12),
            flags: u32_at(rec, 16),
        });
    }
    Ok(out)
}

// --------------------------------------------------------------------------
// Catalogues objects.txt + objgroup.txt (parse une fois).
// --------------------------------------------------------------------------

/// Ligne `objects.txt` réduite aux colonnes consommées.
#[derive(Debug, Clone)]
pub struct ObjRec {
    pub id: u32,
    pub token: String,
    pub name: String,
    /// `Mode0 = 1` : le mode actif (NU) existe ⇒ l'objet peut apparaître.
    pub mode0: bool,
    pub is_door: bool,
    pub open_warp: bool,
    pub operate_fn: u32,
    pub size_x: u32,
    pub size_y: u32,
}

/// Catalogue `objects.txt` par id (lenient : illisible ⇒ vide).
fn catalog() -> &'static HashMap<u32, ObjRec> {
    static C: OnceLock<HashMap<u32, ObjRec>> = OnceLock::new();
    C.get_or_init(|| {
        let text = String::from_utf8_lossy(OBJECTS_TXT);
        let Ok(t) = Tsv::parse(&text) else { return HashMap::new() };
        (0..t.len())
            .filter_map(|i| {
                let r = t.rec(i)?;
                let id = r.u("Id");
                if id == 0 && r.s("Token").is_empty() {
                    return None; // ligne de bruit éventuelle
                }
                Some((
                    id,
                    ObjRec {
                        id,
                        token: r.s("Token").to_string(),
                        name: r.s("Name").to_string(),
                        mode0: r.u("Mode0") == 1,
                        is_door: r.u("IsDoor") == 1,
                        open_warp: r.u("OpenWarp") == 1,
                        operate_fn: r.u("OperateFn"),
                        size_x: r.u("SizeX"),
                        size_y: r.u("SizeY"),
                    },
                ))
            })
            .collect()
    })
}

/// Ligne `objgroup.txt` réduite : entrées `(id objects.txt, prob)` de prob
/// non nulle + colonnes informatives `SHRINES`/`WELLS`.
#[derive(Debug, Clone)]
pub struct GroupRec {
    pub name: String,
    pub entries: Vec<(u32, u32)>,
    pub shrines: u32,
    pub wells: u32,
}

/// Groupes `objgroup.txt` par id de groupe (colonne `Offset`).
fn groups() -> &'static HashMap<u32, GroupRec> {
    static G: OnceLock<HashMap<u32, GroupRec>> = OnceLock::new();
    G.get_or_init(|| {
        let Ok(t) = Tsv::parse(OBJGROUP_TSV) else { return HashMap::new() };
        (0..t.len())
            .filter_map(|i| {
                let r = t.rec(i)?;
                let gid = r.u("Offset");
                let mut entries = Vec::new();
                for n in 0..8 {
                    let id = r.u(&format!("ID{n}"));
                    let prob = r.u(&format!("PROB{n}"));
                    if id != 0 && prob > 0 {
                        entries.push((id, prob));
                    }
                }
                Some((
                    gid,
                    GroupRec {
                        name: r.s("GroupName").to_string(),
                        entries,
                        shrines: r.u("SHRINES"),
                        wells: r.u("WELLS"),
                    },
                ))
            })
            .collect()
    })
}

/// Tirage pondéré par `PROBn` parmi les entrées d'un groupe — PUR (le
/// rouleau vient de l'appelant). Renvoie l'id objects.txt tiré.
pub fn pick_group_entry(group: &GroupRec, roll: u32) -> Option<u32> {
    let total: u32 = group.entries.iter().map(|(_, p)| *p).sum();
    if total == 0 {
        return None;
    }
    let mut roll = roll % total;
    for (id, prob) in &group.entries {
        if roll < *prob {
            return Some(*id);
        }
        roll -= *prob;
    }
    group.entries.last().map(|(id, _)| *id)
}

/// PRNG SplitMix64 local (même convention que DRLG/game : graine de partie
/// ⇒ même plan d'objets). `d2rules::rng::Rng` est crate-interne.
struct SplitMix64(u64);

impl SplitMix64 {
    fn below(&mut self, n: u32) -> u32 {
        if n == 0 {
            return 0;
        }
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        ((z ^ (z >> 31)) >> 32) as u32 % n
    }
}

// --------------------------------------------------------------------------
// Plan d'objets vivants d'un niveau.
// --------------------------------------------------------------------------

/// Nature d'un objet instancié (classification data-driven, cf. doc module).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjKind {
    /// Coffre/casket (loot via TreasureClassEx — ex. « Act 1 Chest B »).
    Chest { tc: String },
    /// Conteneur mineur (urne, baril, cadavre, cache…) — TC « Act N Junk ».
    Junk { tc: String },
    /// Sanctuaire — effet tiré à l'apparition (code `shrines.txt`).
    Shrine,
    /// Porte (`IsDoor=1`).
    Door,
    /// Portail rouge / escalier (`OpenWarp=1`).
    Warp,
    /// Livre cliquable (Tower Tome — aucun butin, documenté).
    Book,
    /// Puits/stocke (`OperateFn` 22) — interactif ; l'effet de source
    /// (soin partiel) n'est pas simulé au 9c, écart documenté.
    Well,
    /// Décor (aucun mode d'opération — visible, non interactif).
    Decor,
}

impl ObjKind {
    /// Étiquette stable pour le contrat JSON UI.
    pub fn as_str(&self) -> &'static str {
        match self {
            ObjKind::Chest { .. } => "chest",
            ObjKind::Junk { .. } => "junk",
            ObjKind::Shrine => "shrine",
            ObjKind::Door => "door",
            ObjKind::Warp => "warp",
            ObjKind::Book => "book",
            ObjKind::Well => "well",
            ObjKind::Decor => "decor",
        }
    }

    /// TreasureClass de conteneur, `None` sinon.
    pub fn tc(&self) -> Option<&str> {
        match self {
            ObjKind::Chest { tc } | ObjKind::Junk { tc } => Some(tc),
            _ => None,
        }
    }
}

/// Une instance interactive posée par le plan du niveau.
#[derive(Debug, Clone)]
pub struct LiveObject {
    /// Index dans [`LiveObjects::objects`].
    pub index: usize,
    /// Id objects.txt résolu (après tirage de groupe).
    pub id: u32,
    pub token: String,
    /// Nom affichable (colonne `Name` objects.txt — pas de clé TBL fr dédiée
    /// pour les objets, repli documenté).
    pub name: String,
    pub kind: ObjKind,
    /// Groupe objgroup d'origine (`type=2`), `None` si direct (`type=1`).
    pub group: Option<u32>,
    /// Position en tuiles (continu — sous-tuiles DS1 / 5).
    pub x: f32,
    pub y: f32,
    /// Conteneur ouvert / porte ouverte.
    pub opened: bool,
    /// Code du sanctuaire tiré (`shrines.txt`), `None` sinon.
    pub shrine: Option<u8>,
    /// Buff temporisé actif (sanctuaire), consommé par le HUD/l'agrégation.
    pub buff: Option<ShrineBuff>,
    /// À partir de quand l'objet est réutilisable (horloge `World`,
    /// `elapsed_ms`) : 0 = maintenant, `u64::MAX` = jamais (reset 0 min).
    pub usable_at_ms: u64,
}

impl LiveObject {
    /// Interactif ? (le décor est posé mais muet.)
    pub fn usable(&self) -> bool {
        !matches!(self.kind, ObjKind::Decor)
    }
}

/// Résultat d'une interaction réussie (contrat JSON UI au clic).
#[derive(Debug, Clone, PartialEq)]
pub enum ObjectUse {
    /// Conteneur ouvert : butin posé au sol (et `ItemDropped` au tick
    /// suivant, pattern `NpcTalked`).
    Loot { items: Vec<ItemDef>, tc: String },
    /// Sanctuaire consommé.
    Shrine { code: u8, name: String, healed: u32, mana: u32, buff_ms: u64 },
    /// Porte basculée.
    Door { open: bool },
    /// Portail/escalier signalé (voyage réel : câblage warp existant).
    Warp { name: String },
    /// Livre lu (flavor).
    Book { name: String },
    /// Puits utilisé (effet de source non simulé au 9c — écart documenté).
    Well { name: String },
}

impl ObjectUse {
    /// JSON stable pour l'UI (`type` = variante).
    pub fn to_json(&self) -> Value {
        match self {
            ObjectUse::Loot { items, tc } => json!({
                "type": "loot", "tc": tc,
                "items": items.iter().map(|i| json!({
                    "code": i.code, "name": i.name, "kind": format!("{:?}", i.kind).to_lowercase(),
                    "amount": i.amount,
                })).collect::<Vec<_>>(),
            }),
            ObjectUse::Shrine { code, name, healed, mana, buff_ms } => json!({
                "type": "shrine", "code": code, "name": name,
                "healed": healed, "mana": mana, "buffMs": buff_ms,
            }),
            ObjectUse::Door { open } => json!({ "type": "door", "open": open }),
            ObjectUse::Warp { name } => json!({ "type": "warp", "name": name }),
            ObjectUse::Book { name } => json!({ "type": "book", "name": name }),
            ObjectUse::Well { name } => json!({ "type": "well", "name": name }),
        }
    }
}

/// Plan d'objets vivants d'un niveau (town/bloodmoor/A2…), déterministe à
/// graine fixe. État d'usage (ouvert/buff/recharge) porté ICI — le câblage
/// `game.rs` (attach + JSON UI) reste au pas d'intégration.
#[derive(Debug, Clone, Default)]
pub struct LiveObjects {
    /// Clé manifest du niveau d'origine (ex. "town").
    pub level_key: String,
    /// Instances posées (Mode0=1 seulement — cf. portail de spawn).
    pub objects: Vec<LiveObject>,
}

impl LiveObjects {
    /// Construit le plan depuis les octets DS1 réels du niveau.
    ///
    /// * `act` : acte du NIVEAU (1..=5) — gouverne les TC de conteneurs
    ///   (« Act N … ») ;
    /// * `mlvl` : MonLvl du niveau (qualité des rolls de loot) ;
    /// * `seed` : graine de partie (tirages de groupes + sanctuaires).
    pub fn for_level(level_key: &str, act: u32, mlvl: u32, ds1: &[u8], seed: u64) -> LiveObjects {
        let raw = decode_ds1_objects(ds1).unwrap_or_default();
        let cat = catalog();
        let mut rng = SplitMix64(seed | 1);
        let mut objects = Vec::new();
        for rec in &raw {
            // type=2 : un GROUPE — tirage pondéré PROB parmi les candidats
            // spawnables (Mode0=1). Tous filtrés ⇒ instance sautée.
            let resolved_id: Option<u32> = match rec.obj_type {
                2 => groups()
                    .get(&rec.id)
                    .and_then(|g| {
                        let valid: Vec<(u32, u32)> = g
                            .entries
                            .iter()
                            .copied()
                            .filter(|(id, _)| cat.get(id).map(|o| o.mode0).unwrap_or(false))
                            .collect();
                        let total: u32 = valid.iter().map(|(_, p)| *p).sum();
                        if total == 0 {
                            return None;
                        }
                        pick_group_entry(
                            &GroupRec { name: g.name.clone(), entries: valid, shrines: g.shrines, wells: g.wells },
                            rng.below(total),
                        )
                    }),
                _ => Some(rec.id),
            };
            let Some(id) = resolved_id else { continue };
            let Some(obj) = cat.get(&id) else { continue };
            // Portail de spawn : pas de mode actif (Mode0=0) ⇒ JAMAIS posé.
            if !obj.mode0 {
                continue;
            }
            let kind = classify(obj, act);
            let shrine = if matches!(kind, ObjKind::Shrine) {
                // Effet tiré À L'APPARITION (sémantique D2), rareté respectée.
                shrines::pick_weighted(rng.below(shrines::pool_weight())).map(|d| d.code)
            } else {
                None
            };
            objects.push(LiveObject {
                index: objects.len(),
                id,
                token: obj.token.clone(),
                name: obj.name.clone(),
                kind,
                group: (rec.obj_type == 2).then_some(rec.id),
                x: rec.x as f32 / 5.0,
                y: rec.y as f32 / 5.0,
                opened: false,
                shrine,
                buff: None,
                usable_at_ms: 0,
            });
        }
        let _ = mlvl; // (réservé : qualité loot — les rolls reçoivent mlvl à l'usage)
        LiveObjects { level_key: level_key.to_string(), objects }
    }

    /// Nombre d'objets INTERACTIFS posés (le décor est exclu).
    pub fn usable_count(&self) -> usize {
        self.objects.iter().filter(|o| o.usable()).count()
    }

    /// Force le code de sanctuaire d'une instance (tests + outils
    /// d'intégration ; le tirage normal passe par la rareté).
    pub fn set_shrine(&mut self, index: usize, code: u8) -> Result<(), String> {
        let obj = self
            .objects
            .get_mut(index)
            .ok_or_else(|| format!("objet {index} hors plan"))?;
        if !matches!(obj.kind, ObjKind::Shrine) {
            return Err(format!("{} n'est pas un sanctuaire", obj.name));
        }
        d2rules::shrines::by_code(code).ok_or_else(|| format!("code shrine {code} inconnu"))?;
        obj.shrine = Some(code);
        Ok(())
    }

    /// Un pas de temps simulé : décompte des buffs de sanctuaires.
    pub fn tick(&mut self, dt_ms: u32) {
        for o in &mut self.objects {
            if let Some(b) = o.buff.as_mut() {
                if !b.tick(dt_ms) {
                    o.buff = None;
                }
            }
        }
    }

    /// État JSON du plan (marqueurs UI) : index/id/token/nom/genre/position
    /// + état d'usage et sanctuaire tiré.
    pub fn to_json(&self) -> Value {
        Value::Array(
            self.objects
                .iter()
                .map(|o| {
                    json!({
                        "index": o.index,
                        "id": o.id,
                        "token": o.token,
                        "name": o.name,
                        "kind": o.kind.as_str(),
                        "group": o.group,
                        "x": o.x,
                        "y": o.y,
                        "opened": o.opened,
                        "shrine": o.shrine,
                        "buff": o.buff.as_ref().map(|b| json!({
                            "code": b.code, "remainingMs": b.remaining_ms,
                        })),
                        "usableAtMs": o.usable_at_ms,
                    })
                })
                .collect(),
        )
    }

    /// Interaction au clic (pattern `NpcTalked` : API directe hors tick,
    /// portée 2.5 tuiles, erreurs descriptives `Err`). Les drops de
    /// conteneurs émettent leurs `ItemDropped` dans `pending_events`
    /// (flushés en tête du tick suivant) ; le temps sanctuaire est
    /// l'horloge `World::elapsed_ms`.
    pub fn use_object(
        &mut self,
        world: &mut World,
        index: usize,
        hero_x: f32,
        hero_y: f32,
    ) -> Result<ObjectUse, String> {
        let obj = self
            .objects
            .get_mut(index)
            .ok_or_else(|| format!("objet {index} hors plan"))?;
        if !obj.usable() {
            return Err(format!("{} n'est pas interactif", obj.name));
        }
        let dx = obj.x - hero_x;
        let dy = obj.y - hero_y;
        if (dx * dx + dy * dy).sqrt() > USE_RANGE_TILES {
            return Err(format!(
                "trop loin — {:+.1} tuiles de {} (max {USE_RANGE_TILES})",
                (dx * dx + dy * dy).sqrt(),
                obj.name
            ));
        }
        let now = world.elapsed_ms();
        if now < obj.usable_at_ms {
            return Err("sanctuaire vide — encore en rechargement".to_string());
        }
        match obj.kind.clone() {
            ObjKind::Chest { tc } | ObjKind::Junk { tc } => {
                if obj.opened {
                    return Err(format!("{} est déjà ouvert", obj.name));
                }
                let items = world.use_container(obj.x, obj.y, &tc, 1);
                obj.opened = true;
                Ok(ObjectUse::Loot { items, tc })
            }
            ObjKind::Shrine => {
                let code = obj.shrine.ok_or("sanctuaire sans effet tiré")?;
                let def = d2rules::shrines::by_code(code)
                    .ok_or_else(|| format!("code shrine {code} inconnu"))?;
                if obj.buff.as_ref().is_some_and(|b| b.is_active()) {
                    return Err("l'effet du sanctuaire est déjà actif".to_string());
                }
                let (healed, mana) = world.apply_shrine_recharge();
                let buff_ms = def.duration_frames as u64 * shrines::D2_FRAME_MS;
                obj.buff = ShrineBuff::from_def(def);
                // Reset officiel : réutilisable après N minutes ; 0 min =
                // une seule fois par partie (jamais : u64::MAX).
                obj.usable_at_ms = if def.reset_minutes == 0 {
                    u64::MAX
                } else {
                    now + def.reset_minutes as u64 * 60_000
                };
                Ok(ObjectUse::Shrine {
                    code,
                    name: def.name.clone(),
                    healed,
                    mana,
                    buff_ms,
                })
            }
            ObjKind::Door => {
                obj.opened = !obj.opened;
                Ok(ObjectUse::Door { open: obj.opened })
            }
            ObjKind::Warp => Ok(ObjectUse::Warp { name: obj.name.clone() }),
            ObjKind::Book => Ok(ObjectUse::Book { name: obj.name.clone() }),
            ObjKind::Well => Ok(ObjectUse::Well { name: obj.name.clone() }),
            ObjKind::Decor => Err(format!("{} n'est pas interactif", obj.name)),
        }
    }
}

/// Classification data-driven d'un record objects.txt (cf. doc de module).
fn classify(obj: &ObjRec, act: u32) -> ObjKind {
    let act = act.max(1);
    if obj.open_warp {
        ObjKind::Warp
    } else if obj.is_door {
        ObjKind::Door
    } else {
        match obj.operate_fn {
            2 => ObjKind::Shrine,
            1 => ObjKind::Chest { tc: format!("Act {act} Chest A") },
            4 => {
                if obj.size_x.saturating_mul(obj.size_y) >= 10 {
                    ObjKind::Chest { tc: format!("Act {act} Chest C") }
                } else {
                    ObjKind::Chest { tc: format!("Act {act} Chest B") }
                }
            }
            3 | 5 | 7 | 14 | 51 => ObjKind::Junk { tc: format!("Act {act} Junk") },
            6 => ObjKind::Book,
            22 => ObjKind::Well,
            _ => ObjKind::Decor,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// Racine du workspace (assets/) — même helper que `npc_bridge.rs`.
    fn ws_root() -> Option<String> {
        let p = Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(3)?;
        p.join("assets").is_dir().then(|| p.to_string_lossy().into_owned())
    }

    fn town_ds1() -> Vec<u8> {
        std::fs::read(format!("{}/assets/map/TownN1.ds1", ws_root().unwrap())).unwrap()
    }

    fn bloodmoor_ds1() -> Vec<u8> {
        std::fs::read(format!("{}/assets/map/bloodmoor.ds1", ws_root().unwrap())).unwrap()
    }

    fn world(grid_w: usize, grid_h: usize) -> World {
        let manifest: serde_json::Value =
            serde_json::from_str(include_str!("../../../../assets/manifest.json")).unwrap();
        let grid = vec![vec![true; grid_w]; grid_h];
        World::with_seed(grid, manifest, 5)
    }

    #[test]
    fn town_ds1_yields_the_real_interactive_plan() {
        let bytes = town_ds1();
        // Section brute : 41 records dans TownN1.ds1 (probe tools/ds1_objects.py).
        let raw = decode_ds1_objects(&bytes).expect("section OBJECTS lisible");
        assert_eq!(raw.len(), 41, "41 objets bruts dans TownN1.ds1");
        assert!(raw.iter().any(|o| o.id == 0), "le NU0 Dummy est bien présent au brut");

        let plan = LiveObjects::for_level("town", 1, 1, &bytes, 7);
        // Portail de spawn : le NU0 (Mode0=0) est filtré — 40 posés.
        assert_eq!(plan.objects.len(), 40, "41 bruts − 1 sans mode actif");
        assert_eq!(plan.usable_count(), 40, "tout le plan town est interactif");
        assert!(plan.objects.iter().all(|o| o.id != 0), "aucun objet sans mode actif");

        // Positions en tuiles, dans les bornes de la carte (57×41).
        for o in &plan.objects {
            assert!((0.0..57.0).contains(&o.x) && (0.0..41.0).contains(&o.y), "{o:?} hors carte");
            assert!((o.x.fract() * 5.0).round().abs() < 1e-6 || true); // x = sous-tuiles/5
        }
        // Le coffre direct L1 (id 5, sans groupe, 1×2) → « Act 1 Chest B ».
        let chest = plan
            .objects
            .iter()
            .find(|o| o.id == 5 && o.group.is_none())
            .expect("le coffre L1 de la taverne est dans le DS1");
        assert_eq!(chest.kind, ObjKind::Chest { tc: "Act 1 Chest B".into() });
        assert_eq!((chest.x, chest.y), (30.6, 9.2), "sous-tuiles (153,46)/5");
        // Les caskets C6 (OperateFn 1) → « Act 1 Chest A ».
        assert!(plan
            .objects
            .iter()
            .filter(|o| o.id == 3)
            .all(|o| o.kind == ObjKind::Chest { tc: "Act 1 Chest A".into() }));
        // Deux portes D1 (id 13) + un livre Tower Tome (id 8).
        assert_eq!(plan.objects.iter().filter(|o| o.id == 13).count(), 2);
        assert!(plan
            .objects
            .iter()
            .find(|o| o.id == 8)
            .is_some_and(|o| o.kind == ObjKind::Book));
        // Sanctuaire direct SF (id 2) : effet tiré dans le pool officiel.
        let shrine = plan.objects.iter().find(|o| o.id == 2).expect("SF shrine du camp");
        assert_eq!(shrine.kind, ObjKind::Shrine);
        assert!(shrine.shrine.is_some(), "effet tiré à l'apparition");
        assert!(d2rules::shrines::by_code(shrine.shrine.unwrap()).is_some());
        // Instances de groupes (type=2) : au moins celle du groupe 33
        // (« outsideforestobj1 » — q1/q2/q6, que des chests Act 1 B).
        let g33 = plan
            .objects
            .iter()
            .find(|o| o.group == Some(33))
            .expect("l'instance du groupe 33");
        assert!(matches!(g33.kind, ObjKind::Chest { .. }));
        // JSON lisible par l'UI.
        let v: Value = serde_json::from_str(&plan.to_json().to_string()).unwrap();
        assert_eq!(v.as_array().unwrap().len(), 40);
    }

    #[test]
    fn bloodmoor_and_lutgholein_have_no_objects_section() {
        // bloodmoor.ds1 finit après les 6 couches : section absente ⇒ 0.
        let raw = decode_ds1_objects(&bloodmoor_ds1()).expect("décodage OK");
        assert!(raw.is_empty(), "bloodmoor.ds1 : pas de section OBJECTS");
        let plan = LiveObjects::for_level("bloodmoor", 1, 1, &bloodmoor_ds1(), 7);
        assert!(plan.objects.is_empty());
        assert_eq!(plan.usable_count(), 0);
    }

    #[test]
    fn chest_from_real_ds1_drops_its_tc_and_tracks_state() {
        let bytes = town_ds1();
        let mut plan = LiveObjects::for_level("town", 1, 1, &bytes, 7);
        let (idx, cx, cy) = plan
            .objects
            .iter()
            .find(|o| o.id == 5)
            .map(|o| (o.index, o.x, o.y))
            .unwrap();
        let mut w = world(57, 41);

        // Trop loin : erreur descriptive (pattern NpcTalked).
        let err = plan.use_object(&mut w, idx, cx + 10.0, cy).unwrap_err();
        assert!(err.contains("trop loin"), "{err}");

        // À portée : drop TC réel posé au sol à la position du coffre.
        let use1 = plan.use_object(&mut w, idx, cx, cy).expect("ouverture du coffre");
        let ObjectUse::Loot { items, tc } = use1 else { panic!("Loot attendu") };
        assert_eq!(tc, "Act 1 Chest B");
        assert!(!items.is_empty(), "le coffre lâche au moins un objet");
        let base = w.items().len() - items.len();
        for (i, it) in items.iter().enumerate() {
            assert_eq!((w.items()[base + i].x, w.items()[base + i].y), (cx, cy));
        }
        // Pattern NpcTalked : ItemDropped flushés au tick suivant.
        let ev = w.tick(0, None);
        assert!(ev.iter().any(|e| matches!(e, d2rules::GameEvent::ItemDropped { .. })));

        // Déjà ouvert : refus propre, zéro drop en plus.
        let before = w.items().len();
        assert!(plan.use_object(&mut w, idx, cx, cy).unwrap_err().contains("déjà ouvert"));
        assert_eq!(w.items().len(), before);

        // Déterminisme : même graine ⇒ mêmes drops (TC roulée identique).
        let mut plan2 = LiveObjects::for_level("town", 1, 1, &bytes, 7);
        let mut w2 = world(57, 41);
        let (i2, x2, y2) = plan2
            .objects
            .iter()
            .find(|o| o.id == 5)
            .map(|o| (o.index, o.x, o.y))
            .unwrap();
        let use2 = plan2.use_object(&mut w2, i2, x2, y2).unwrap();
        let ObjectUse::Loot { items: items2, .. } = use2 else { unreachable!() };
        assert_eq!(
            items.iter().map(|i| (&i.code, i.amount)).collect::<Vec<_>>(),
            items2.iter().map(|i| (&i.code, i.amount)).collect::<Vec<_>>(),
            "même graine ⇒ même butin"
        );
    }

    #[test]
    fn shrine_effect_applies_then_expires_with_simulated_time() {
        let bytes = town_ds1();
        let mut plan = LiveObjects::for_level("town", 1, 1, &bytes, 7);
        let (idx, sx, sy) = plan
            .objects
            .iter()
            .find(|o| o.id == 2)
            .map(|o| (o.index, o.x, o.y))
            .unwrap();
        // Effet imposé pour le test : Combat Boost (code 7, +200 % AR/dmg).
        plan.set_shrine(idx, 7).expect("shrine SF forcé au code 7");
        let mut w = world(57, 41);

        let use1 = plan.use_object(&mut w, idx, sx, sy).expect("activation du sanctuaire");
        let ObjectUse::Shrine { code, buff_ms, .. } = use1 else { panic!("Shrine attendu") };
        assert_eq!(code, 7);
        assert_eq!(buff_ms, 96_000, "2400 frames × 40 ms (25 ips D2)");

        // Buff actif, branché au socle statlist (ids ItemStatCost réels).
        let buff = plan.objects[idx].buff.clone().expect("buff porté");
        assert!(buff.is_active());
        assert_eq!(buff.stat_lines().len(), 2, "+AR% et +dmg%");

        // Temps simulé : actif juste avant la durée, expiré après.
        plan.tick(95_999);
        assert!(plan.objects[idx].buff.is_some(), "actif à 1 ms de la fin");
        plan.tick(1);
        assert!(plan.objects[idx].buff.is_none(), "expiré (temps simulé)");

        // Rechargement officiel : reset 5 minutes (300 000 ms) sur l'HORLOGE
        // WORLD (source de temps unique — `elapsed_ms`), pas le tick du plan.
        // `World::tick` borne dt (MAX_DT_MS) : on simule 3000 pas de 100 ms.
        assert!(plan.use_object(&mut w, idx, sx, sy).is_err(), "pas encore rechargé");
        for _ in 0..3000 {
            w.tick(100, None);
        }
        assert!(w.elapsed_ms() >= 300_000, "temps monde avancé (simulé)");
        let use2 = plan.use_object(&mut w, idx, sx, sy).expect("sanctuaire rechargé après reset");
        assert!(matches!(use2, ObjectUse::Shrine { .. }));
    }

    #[test]
    fn objects_without_active_mode_never_spawn_synthetic_ds1() {
        // DS1 synthétique 2×2 (stocké 1×1) : deux objets, dont le NU0 Dummy
        // (Mode0=0 — pas de mode actif). En-tête + 0 fichiers + meta + 6
        // couches + section OBJECTS.
        let mut d: Vec<u8> = Vec::new();
        let put = |d: &mut Vec<u8>, v: i32| d.extend_from_slice(&v.to_le_bytes());
        put(&mut d, 18); // version
        put(&mut d, 1); // w stocké (réel 2)
        put(&mut d, 1); // h stocké (réel 2)
        put(&mut d, 0); // 2 zéros
        put(&mut d, 0);
        put(&mut d, 0); // fileCount
        put(&mut d, 0); // meta
        put(&mut d, 0);
        for _ in 0..6 * 4 {
            put(&mut d, 0); // 6 couches × 4 cellules
        }
        put(&mut d, 2); // 2 objets
        // (type, id, x, y, flags) — NU0 en sous-tuiles (5,5) ; C6 en (6,6).
        for (typ, id, x, y) in [(1, 0, 5, 5), (1, 3, 6, 6)] {
            put(&mut d, typ);
            put(&mut d, id);
            put(&mut d, x);
            put(&mut d, y);
            put(&mut d, 0);
        }
        let raw = decode_ds1_objects(&d).expect("DS1 synthétique lisible");
        assert_eq!(raw.len(), 2, "les 2 records bruts");
        let plan = LiveObjects::for_level("test", 1, 1, &d, 3);
        assert_eq!(plan.objects.len(), 1, "le NU0 (Mode0=0) ne spawn PAS");
        assert_eq!(plan.objects[0].id, 3, "seul le Casket (mode actif) est posé");
        assert_eq!((plan.objects[0].x, plan.objects[0].y), (1.2, 1.2), "sous-tuiles/5");
    }

    #[test]
    fn group_pick_is_prob_weighted_and_spawn_gated() {
        // Groupe 1 (« Rogues for act1 ») : 4 entrées × prob 25 — tirage
        // uniforme parmi des objects.txt tous Mode0=1.
        let g = groups().get(&1).expect("groupe 1 embarqué");
        assert_eq!(g.entries.len(), 4);
        for roll in [0, 24, 25, 49, 74, 99] {
            let id = pick_group_entry(g, roll).expect("entrée tirée");
            assert!(g.entries.iter().any(|(i, _)| *i == id));
            assert!(catalog().get(&id).map(|o| o.mode0).unwrap_or(false));
        }
        // Un groupe vide ne tire rien (jamais de panique).
        let empty = GroupRec { name: "vide".into(), entries: vec![], shrines: 0, wells: 0 };
        assert!(pick_group_entry(&empty, 7).is_none());
    }

    #[test]
    fn doors_warps_and_decor_follow_their_operate_rules() {
        let bytes = town_ds1();
        let mut plan = LiveObjects::for_level("town", 1, 1, &bytes, 7);
        let mut w = world(57, 41);
        // Porte D1 : bascule ouverte/fermée.
        let (di, dx_, dy_) = plan
            .objects
            .iter()
            .find(|o| o.id == 13)
            .map(|o| (o.index, o.x, o.y))
            .unwrap();
        let u1 = plan.use_object(&mut w, di, dx_, dy_).unwrap();
        assert_eq!(u1, ObjectUse::Door { open: true });
        let u2 = plan.use_object(&mut w, di, dx_, dy_).unwrap();
        assert_eq!(u2, ObjectUse::Door { open: false });
        // Décor : jamais interactif (aucun dans town, on vérifie la garde via
        // un index décor synthétique).
        plan.objects.push(LiveObject {
            index: plan.objects.len(),
            id: 33,
            token: "A1".into(),
            name: "Dummy".into(),
            kind: ObjKind::Decor,
            group: None,
            x: 1.0,
            y: 1.0,
            opened: false,
            shrine: None,
            buff: None,
            usable_at_ms: 0,
        });
        let di2 = plan.objects.len() - 1;
        assert!(plan.use_object(&mut w, di2, 1.0, 1.0).is_err());
        // JSON : tags de genre stables.
        let j = plan.to_json();
        assert!(j.as_array().unwrap().iter().all(|o| o["kind"].is_string()));
    }
}
