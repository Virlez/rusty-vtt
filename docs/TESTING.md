# Tests

Prérequis : Rust stable, Node 24+, puis `npm ci` et `npx playwright install chromium`.

`npm test` exécute toute la pyramide, dans l'ordre :

| Niveau | Commande | Couverture |
| --- | --- | --- |
| Unitaire et règles métier Rust | `npm run test:unit` | Dés, projections privées, combat, états, géométrie, créateur 5e, configuration et quotas |
| Intégration HTTP réelle | `npm run test:integration` | Cinq suites : smoke, builder, advanced, QA et sécurité ; persistance et redémarrage compris |
| E2E Chromium | `npm run test:e2e` | Sessions, deux clients SSE, dés publics/privés, fiches et initiative, journal, objets, ambiance, canvas, raccourcis, scènes, import/export et brouillons 5e |

Les E2E utilisent le vrai binaire Rust et les vrais échanges HTTP/SSE, sans simuler le backend. Chaque test démarre son serveur sur un port attribué par le système et un monde temporaire supprimé après le test. Les sessions MJ et joueur ont des contextes navigateur distincts. Les suites HTTP historiques utilisent leurs ports dédiés et sont exécutées séquentiellement. Aucun test ne lit `data/world.json` de votre campagne.

`RUSTY_TEST_BINARY` permet de choisir un binaire précompilé. Les commandes npm compilent normalement `target/debug/rusty-vtt` (avec `.exe` sous Windows). Éviter de recompiler pendant une suite : Windows verrouille les exécutables en cours d'utilisation.

Pour travailler sur les E2E : `npm run test:e2e:ui`, ou `npx playwright test --grep "journal"`. Le mode UI suppose le binaire déjà compilé. Consulter les diagnostics avec `npm run test:e2e:report` ; captures et traces sont conservées sur échec. La CI installe Chromium et exécute `npm test`, puis publie les diagnostics en cas d'échec.

La couverture navigateur cible Chromium. La matrice Firefox/WebKit, les comparaisons visuelles, la lecture audio effective, la création 5e exhaustive et les gestes tactiles restent à compléter. Les règles détaillées du créateur, des portes, des collisions et des états sont surtout vérifiées dans les couches Rust et HTTP pour conserver une suite E2E rapide.

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

Le workflow **Mutation tests** se lance sur les PR qui touchent le code ou ses
données embarquées, ainsi que manuellement avec `workflow_dispatch`. Une mutation
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
