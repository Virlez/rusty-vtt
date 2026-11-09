# Tests

Prérequis : Rust stable, Node 24+, puis `npm ci` et `npx playwright install --with-deps chromium firefox webkit`.

`npm test` exécute toute la pyramide, dans l'ordre :

| Niveau | Commande | Couverture |
| --- | --- | --- |
| Unitaire et règles métier Rust | `npm run test:unit` | Dés, projections privées, combat, états, géométrie, créateur 5e, configuration et quotas ; neuf propriétés générées |
| Intégration HTTP réelle | `npm run test:integration` | Neuf suites : smoke, builder, advanced, QA, sécurité, confidentialité, persistance, régressions 5e et grands mondes |
| E2E Chromium, Firefox et WebKit | `npm run test:e2e` | Sessions, deux clients SSE, dés publics/privés, fiches et initiative, journal, objets, ambiance, canvas, raccourcis, scènes, import/export et brouillons 5e |

Les E2E utilisent le vrai binaire Rust et les vrais échanges HTTP/SSE, sans simuler le backend. Chaque test démarre son serveur sur un port attribué par le système et un monde temporaire supprimé après le test. Les sessions MJ et joueur ont des contextes navigateur distincts. Les suites HTTP historiques utilisent leurs ports dédiés et sont exécutées séquentiellement. Aucun test ne lit `data/world.json` de votre campagne.

`RUSTY_TEST_BINARY` permet de choisir un binaire précompilé. Les commandes npm compilent normalement `target/debug/rusty-vtt` (avec `.exe` sous Windows). Éviter de recompiler pendant une suite : Windows verrouille les exécutables en cours d'utilisation.

Pour travailler sur les E2E : `npm run test:e2e:ui`, ou `npx playwright test --grep "journal"`. Le mode UI suppose le binaire déjà compilé. Consulter les diagnostics avec `npm run test:e2e:report` ; captures et traces sont conservées sur échec. La CI installe les trois moteurs et leurs dépendances système, puis exécute `npm test`, puis publie les diagnostics en cas d'échec.

La suite exécute les mêmes 13 scénarios sur Chromium, Firefox et WebKit, soit 39 tests. WebKit vérifie le moteur utilisé par Safari, sans certifier Safari sur macOS/iOS. Les comparaisons visuelles, la lecture audio effective, la création 5e exhaustive et les gestes tactiles restent à compléter. Les règles détaillées du créateur, des portes, des collisions et des états sont surtout vérifiées dans les couches Rust et HTTP pour conserver une suite E2E rapide.

## E2E multi-navigateur

`npm run test:e2e` compile le serveur puis exécute les trois projets Playwright.
Le viewport est commun (1440 × 1000), avec trois workers en local et deux en CI.
Chaque scénario et chaque moteur disposent de leur propre serveur et monde
éphémère. Les contextes MJ et joueur utilisent le moteur du projet courant.

```sh
# Un moteur pour une boucle de développement plus courte :
npm run test:e2e -- --project=firefox
# Les mêmes parcours sur les trois moteurs, sans recompilation :
npx playwright test
# Uniquement la reconnexion SSE :
npx playwright test reconnection.spec.js
```

Le scénario de reconnexion utilise un relais HTTP local réservé au joueur.
Il coupe sa connexion SSE réelle et rend ses rafraîchissements HTTP indisponibles,
pendant que le MJ continue à publier un message public et un jet personnel.
À la reprise du relais, le navigateur doit récupérer le message manqué sans
rechargement ni nouvelle action, conserver sa session, masquer le jet personnel
et reprendre les échanges dans les deux sens. Les nouvelles tentatives SSE sont
retenues pendant l'interruption, puis retransmises au vrai serveur ; aucun état
de monde ni résultat métier n'est simulé. Cela évite de dépendre du comportement
du mode hors ligne de chaque navigateur sur les connexions déjà ouvertes.

La CI conserve les traces et captures en cas d'échec ; les chemins Playwright
identifient le projet (`chromium`, `firefox`, `webkit`). Il n'y a pas d'exclusion
par moteur ni de scénario ignoré. Installer les dépendances système nécessite
les droits correspondants sur Linux ; `--with-deps` s'en charge en CI.

## Confidentialité HTTP/SSE et persistance

Deux suites utilisent le vrai binaire, des ports attribués par le système et un
monde temporaire supprimé en fin de test, via `tests/helpers/server.mjs` :

```sh
cargo build --locked
node tests/privacy.mjs target/debug/rusty-vtt
node tests/persistence.mjs target/debug/rusty-vtt
```

Sous Windows, ajouter `.exe` au chemin du binaire. Ces suites sont incluses dans
`npm run test:integration` et donc dans `npm test` et la CI existante.

- **Confidentialité** : assertions exactes pour le MJ, Alice, Bob et Eve sur les
  réponses de connexion, l'état HTTP et les réponses d'action. Les cas couvrent
  les notes, données Foundry, inventaires importés, builds 5e, brouillons,
  journaux/objets/tables privés, messages et jets aveugles, brouillard, vision,
  pions cachés et références de combat, dans les scènes actives et inactives.
  Les notifications SSE ne contiennent que la révision ; les quatre sessions
  rechargent leur état après chaque événement pour vérifier la projection. Même
  le MJ ne reçoit pas les messages personnels ou murmures d'autres auteurs.
  L'export complet et le fichier sauvegardé doivent conserver les données privées.
- **Persistance** : un JSON tronqué ou une structure invalide bloque le démarrage
  sans écraser le fichier. L'export/import et deux redémarrages conservent le
  monde. Des dossiers placés à la place du fichier temporaire ou de la cible
  provoquent de vrais échecs d'écriture/renommage, pour les actions et les imports.
  Un échec ne publie ni monde, ni révision, ni événement SSE ; une requête suivante
  réussit après retrait du blocage. Douze ajouts simultanés doivent tous être
  conservés une fois, avec des révisions distinctes et consécutives, et un disque
  identique à la mémoire, y compris après redémarrage.

Les pannes sont provoquées sans `chmod`, pour fonctionner aussi avec un utilisateur
privilégié. Cette simulation couvre les erreurs du système de fichiers et la
reprise applicative ; elle ne simule pas une coupure électrique ou la durabilité
physique du stockage.

## Tests par propriétés

`src/property_tests.rs` utilise `proptest`, uniquement comme dépendance de test
avec la fonctionnalité `std`. Les neuf propriétés couvrent les bornes
mathématiques des jets positifs/négatifs, le quota cumulé de dés, les chaînes
Unicode arbitraires sans panic, les bornes numériques et PV fractionnaires,
la translation du brouillard, les traversées de murs dans les deux directions
et selon les deux axes, la projection sans modification de sa source, les
refus de permissions sans mutation et la durée des états actifs/désactivés.
Les jets sont vérifiés par invariants ; aucun test ne dépend d'une fréquence
statistique ou d'un résultat de dé particulier.

```sh
cargo test --locked property_tests
# Plus de cas et une graine reproductible (syntaxe shell POSIX) :
PROPTEST_CASES=1024 PROPTEST_RNG_SEED=12345 cargo test --locked property_tests
```

Par défaut, chaque propriété explore 256 cas avec une graine aléatoire.
En cas d'échec, `proptest` réduit l'entrée et conserve la graine dans
`proptest-regressions/`. Conserver ces fichiers dans Git lorsqu'une régression
est trouvée, avec le correctif, pour rejouer le cas automatiquement. La CI les
joint aux diagnostics en cas d'échec. Les tests par propriétés font partie de
`cargo test` ; la campagne de mutation reste à lancement manuel.

## Tests de mutation du noyau critique

Les tests de mutation vérifient la capacité des assertions à détecter des erreurs :
`cargo-mutants` inverse des conditions, change des opérateurs ou remplace des
retours, puis relance les tests Rust pour chaque variante. Les mutations sont
exécutées dans des copies temporaires ; le code de travail reste intact.

Installer la version utilisée en CI, puis préparer les dépendances :

```sh
cargo install cargo-mutants --version 27.1.0 --locked
cargo fetch --locked
npm run test:mutation -- --jobs 2
```

`cargo mutants --jobs 2` est l'équivalent sans Node/npm. Les dépendances sont
ensuite utilisées en mode `--locked --offline`. Le runner vérifie d'abord que les
tests passent sans mutation ; ne pas utiliser `--baseline skip` en CI.

Le périmètre initial est explicite dans [`.cargo/mutants.toml`](../.cargo/mutants.toml) :

| Risque | Fonctions mutées |
| --- | --- |
| Usurpation ou session expirée | `user`, `session_key`, `session_cookie`, `Session::valid_at` |
| Secrets et exposition HTTP | `players`, `bind_address` |
| Contournement des quotas | `Window::allow`, `Window::expired` |
| Modification de documents non possédés | `owned` |
| Fuite de messages privés et jets aveugles | `chat::project` |
| Brouillard, collision et visibilité | `covered`, `blocks`, `check_move`, `visible` |
| Valeurs hors limites et PV fractionnaires | `num`, `max_hp` |
| Expiration des états de combat | `effects::advance_round` |

Ce périmètre ne mesure pas tout le backend : le créateur 5e, le parseur aléatoire
des dés, la projection complète du monde, les handlers HTTP, la persistance et
le frontend restent couverts par leurs tests habituels et pourront être ajoutés
par étapes. La campagne de mutation n'exécute pas les suites HTTP ou Playwright
pour chaque variante. Elle est séparée de `npm test` pour conserver une boucle
de développement rapide.

Le workflow **Mutation tests** se lance uniquement à la demande avec
`workflow_dispatch` : dans GitHub Actions, sélectionner **Mutation tests**, puis
**Run workflow** et la branche à tester. Il ne se déclenche pas sur les PR. Une mutation
survivante fait échouer le job ; un dépassement du délai de test est également
un échec à analyser. Aucun `continue-on-error` ne masque le résultat. Le rapport
`mutation-report` est conservé 14 jours, y compris lorsque le job échoue.

En local, consulter `mutants.out/missed.txt`, `caught.txt`, `unviable.txt`,
`timeout.txt` et `outcomes.json`, puis les fichiers `diff/` et `log/` pour le détail.
Les mutants non compilables (`unviable`) ne sont pas des erreurs détectées par
les assertions et ne doivent pas être comptés dans un score de détection.
Les rapports sont ignorés par Git.

Pour examiner ou reproduire une famille de mutations :

```sh
cargo mutants --list
cargo mutants --re 'in owned|replace owned'
```

Face à un survivant, ajouter une assertion métier ou un cas limite qui distingue
les comportements, puis relancer la campagne. Une éventuelle exclusion doit
identifier précisément un mutant équivalent et expliquer pourquoi aucune entrée
valide ne permet de le distinguer ; ne pas exclure une fonction pour faire monter
le score.

## Contrôles qualité et couverture en CI

`npm run check` exécute ESLint et Prettier sur les scripts web, tests, outils et
configurations JavaScript. `npm run format` applique le formatage. Les scripts web
historiques partagent encore des variables globales : les règles `no-undef` et
`no-unused-vars` y sont désactivées, contrairement aux tests et outils. Les autres
règles recommandées et l'interdiction d'`eval` restent actives. Le passage à des
imports explicites pourra lever cette exception ; aucun framework n'est requis.

La CI exécute également `cargo clippy --locked --all-targets -- -D warnings`.
Les clones signalés dans les tests par propriétés sont corrigés. Le code JavaScript
existant a été formaté pour permettre une revue et des modifications plus lisibles.

Pour produire le même rapport de couverture Rust que la CI :

```sh
rustup component add llvm-tools-preview
cargo install cargo-llvm-cov --version 0.9.1 --locked
npm run test:coverage
```

`coverage/lcov.info` contient le rapport LLVM brut. `coverage/summary.md` et le
résumé GitHub Actions comptent seulement les lignes applicatives instrumentées,
en excluant les modules de tests internes et les fichiers de tests génératifs et
du créateur. L'artefact `rust-coverage` est conservé sept jours. Il s'agit de la
couverture **unitaire Rust**, sans les suites HTTP/E2E, les branches ni le JavaScript.
Aucun seuil arbitraire ne transforme cette mesure en garantie de qualité.

## Régressions 5e, désynchronisation et clavier

`tests/builder-regressions.mjs` construit une fiche réelle, refuse des imports aux
structures internes incohérentes et vérifie que ni la révision ni le fichier ne
changent. La validation couvre le build, les classes et niveaux, les dés de vie,
les caractéristiques, la configuration et les métadonnées utilisées par les actions
5e. Une fiche 5e incomplète est refusée au lieu d'être réparée implicitement.
Les commandes de progression et de repos vérifient aussi les données avant de
les utiliser. Les nombres extrêmes d'un brouillon donnent des erreurs métier,
sans débordement arithmétique.

`src/builder_tests.rs` teste les douze classes avec des PV et armures attendus
explicitement, les listes de sorts du magicien, le multiclassage et ses prérequis,
le budget d'équipement, la progression et l'amélioration de caractéristique au
niveau 4, avec conservation de l'inventaire et de l'argent.

Les deux scénarios de `resilience.spec.js` s'exécutent sur les trois moteurs :
une ancienne réponse `/api/state` est retardée et livrée après un état plus récent
pour vérifier l'absence de retour en arrière ; un formulaire est ouvert, parcouru,
enregistré et fermé au clavier. Les dialogues principaux utilisent `showModal`,
placent le focus sur le premier champ et contiennent la navigation par Tab.
Ces tests ne constituent pas un audit complet WCAG ou lecteur d'écran.

## Mondes volumineux

`tests/large-world.mjs` importe successivement 1 000 et 10 000 personnages (environ
0,45 et 4,5 Mo). Il vérifie les projections privées, huit écritures concurrentes,
les révisions et la cohérence disque/mémoire. Les requêtes ont une limite de 30 s
pour distinguer les blocages des variations de vitesse des runners. La médiane
et le p95 du petit échantillon de lectures/actions sont imprimés à titre indicatif ;
ils ne constituent ni un SLA ni un benchmark de capacité. Les mondes sont isolés
et supprimés. La suite est incluse dans `npm test` et reste bornée à 120 s.
Le verrou global et la sauvegarde complète restent en place : toute optimisation
future devra être comparée à ces mesures, sans affaiblir les invariants de persistance.
