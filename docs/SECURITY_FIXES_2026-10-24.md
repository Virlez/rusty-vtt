# Corrections de sécurité — 24 octobre 2026

Les cinq constats de l'audit ont été traités dans les sources. Aucune campagne existante n'a été modifiée pendant les vérifications. La configuration d'accès change : lire la section « Sauvegardes et réseau » du README avant de redémarrer.

| Constat | Correction | Vérification |
| --- | --- | --- |
| SEC-01 : usurpation d'un nom libre | Chaque nom joueur est associé à un secret individuel dans `RUSTY_PLAYERS_JSON`. Secrets distincts, minimum 16 octets, aucun accès anonyme. Les noms configurés restent des identifiants stables pour les propriétaires et les messages existants. | Nom seul, secret d'un autre joueur et nom inconnu refusés ; droits du propriétaire légitime conservés ; notes/messages privés absents pour les autres comptes. |
| SEC-02 : secret MJ public | Secret explicite d'au moins 16 octets, ou génération aléatoire de 128 bits au démarrage. | `rusty-gm` refusé ; le secret généré permet la connexion MJ. |
| SEC-03 : fuite dans le combat | Filtrage des références aux pions après la projection brouillard/vision ; recalcul du tour projeté. | Pions cachés et sous brouillard absents du combat joueur ; vision limitée et tour recalculé couverts par tests unitaires ; projection MJ conservée. |
| SEC-04 : sessions et quotas | Expiration après 8 heures, déconnexion avec révocation serveur, rotation à la reconnexion ; quotas de connexion, session, mutation et SSE ; monde sauvegardé limité à 32 Mio. | Expiration et quota SSE en tests unitaires ; déconnexion, ancien cookie, rotation, quota de sessions, tentatives et mutations en tests HTTP ; un flux SSE ouvert se ferme après révocation. |
| SEC-05 : HTTP réseau | Refus des adresses d'écoute non locales. Mode proxy HTTPS explicite avec cookie Secure, contrôle `X-Forwarded-Proto`, HSTS ; réponses non mises en cache. | Configuration publique IPv4/IPv6 rejetée en tests unitaires ; API sans indication HTTPS refusée en mode proxy ; cookies de connexion et de déconnexion Secure. |

## Configuration et migration

Le propriétaire configure les noms exacts de ses joueurs existants et leur distribue un secret aléatoire individuel. Les secrets résident dans la configuration du processus, jamais dans le monde exporté. Il n'y a pas d'inscription publique ni de mécanisme de récupération autonome. Modifier un nom nécessite aussi de transférer ses documents et de prendre en compte les anciens destinataires de messages ; conserver les noms est recommandé.

Les variables sont chargées au démarrage. Redémarrer après une révocation ou une rotation de secrets invalide toutes les anciennes sessions. Le bouton d'identité déconnecte la session courante. Une session expirée renvoie l'interface à la connexion.

L'accès depuis une autre machine nécessite un proxy TLS correctement déployé ; `RUSTY_HTTPS=1` ne fournit pas TLS à lui seul. Le proxy doit remplacer l'en-tête de protocole, rediriger HTTP et garder le backend inaccessible depuis le réseau. Le serveur ne fait pas confiance aux en-têtes d'IP cliente pour les quotas ; tous les participants derrière un même proxy partagent donc la fenêtre de connexion de 30 requêtes/minute.

## Validation

- `cargo test --locked` : 17 tests unitaires, dont expiration, limites SSE et projection du combat.
- `cargo clippy --locked --all-targets -- -D warnings` et `cargo fmt --check`.
- Scénario `tests/security-audit.mjs`, transformé en assertions de protection.
- Parcours HTTP `smoke`, `builder-smoke`, `advanced`, `qa-audit` adaptés aux secrets configurés, sur mondes temporaires.

Les essais HTTPS simulent l'en-tête transmis par un proxy local et vérifient les décisions du backend. Aucun certificat ni véritable déploiement TLS n'a été installé ou testé. L'audit RustSec des dépendances reste à réaliser (`cargo audit` absent). Les correctifs ne changent pas la limite documentée du brouillard : une image de fond complète est toujours téléchargée par le navigateur.
