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
