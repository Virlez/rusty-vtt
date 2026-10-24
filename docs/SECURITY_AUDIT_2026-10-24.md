# Audit de sécurité — Rusty VTT — 24 octobre 2026

> Ce document conserve les constats de l'audit initial. Les cinq points ont depuis été traités : voir [les corrections, les limites et leur validation](SECURITY_FIXES_2026-10-24.md). Les procédures d'exploitation ci-dessous décrivent la version antérieure aux correctifs.

## Conclusion

Le projet convient au modèle annoncé d'une table de confiance sur une machine ou un réseau maîtrisé. Il ne fournit pas une séparation fiable entre joueurs malveillants et ne doit pas être exposé directement sur Internet. Cinq constats sont détaillés ci-dessous, dont deux de sévérité élevée. Les faiblesses d'identité et de configuration par défaut sont en partie documentées dans le README ; leur exploitation a ici été vérifiée.

Audit du code local et essais HTTP sur une instance temporaire, liée exclusivement à `127.0.0.1:3029`. Aucune campagne existante ni service distant testé. Aucun correctif applicatif effectué. Les fichiers QA déjà présents ont été conservés.

## Méthode et résultats

- Lecture des routes, sessions, permissions, projections JSON, import/export, constructeur, chat, effets, vision et usages DOM.
- `cargo test --locked` : **13 tests réussis**.
- `cargo build --locked` : réussi.
- `node tests/security-audit.mjs` : tous les contrôles réussis et constats reproduits. Ce script caractérise les vulnérabilités actuelles : ses assertions devront être inversées après correction pour devenir des tests de non-régression.
- Contrôles négatifs : état sans session → 401 ; modification d'une fiche appartenant à un autre nom → 403 ; export joueur → 403 ; notes et messages privés absents pour un autre nom.
- Recherche ciblée dans les sources : aucun secret de production identifié ; le mot de passe de secours est public. Cela ne constitue pas une analyse de tout l'historique Git.
- `cargo audit` indisponible sur cette machine : **vulnérabilités connues des dépendances non vérifiées**. Le verrouillage des versions ne garantit pas leur innocuité.

La sévérité est qualitative, fondée sur l'impact et les préconditions. Aucune résistance à une charge importante, configuration de proxy, TLS externe ou configuration de déploiement réelle n'a été validée.

## SEC-01 — Élevée — Identité et permissions fondées sur un nom libre

**Sources :** `src/main.rs:183` (login), `src/main.rs:450` (owned), `src/main.rs:65` (projection), `src/chat.rs:56` (confidentialité), `src/builder.rs:101` (brouillons).

Une connexion joueur accepte un nom sans preuve d'identité, y compris celui d'une session existante ou du MJ. Les autorisations comparent ensuite ce nom aux propriétaires et auteurs. Un attaquant ayant accès au serveur peut se connecter comme « Alice », sans mot de passe, pour récupérer les droits d'Alice. Le rôle MJ n'est pas obtenu par le seul nom, mais les documents possédés par ce nom et ses messages personnels deviennent accessibles.

**Preuve reproduite :** créer une fiche avec propriétaire Alice et une note privée ; Alice écrit un message `self` ; Eve ne voit ni note ni message et reçoit 403 sur la modification ; une nouvelle connexion Alice sans mot de passe voit les deux secrets et peut modifier les PV avec HTTP 200. Les propriétaires sont exposés dans l'état joueur, ce qui facilite le choix du nom.

**Correction :** attribuer un identifiant utilisateur stable et authentifié ; utiliser cet identifiant pour propriétaires, destinataires et auteurs. Prévoir un compte ou un jeton d'invitation secret par joueur, la révocation et la migration des propriétaires existants. Refuser uniquement les noms simultanés ne suffit pas : un joueur déconnecté resterait usurpable.

## SEC-02 — Élevée si le serveur est accessible à des tiers — Mot de passe MJ public par défaut

**Source :** `src/main.rs:717` ; comparaison et attribution du rôle dans `src/main.rs:191`.

En l'absence de `RUSTY_GM_PASSWORD`, le serveur utilise `rusty-gm`, publié dans le README. Le problème devient une compromission complète de la campagne dès que l'instance est accessible à une personne non fiable : lecture via export, modification et remplacement du monde par import.

**Preuve reproduite :** instance sans cette variable ; POST `/api/login` avec un nom arbitraire et `password: "rusty-gm"` → HTTP 200, `user.role: "gm"`.

**Correction :** exiger un secret explicite au démarrage ou générer un secret aléatoire propre à l'instance. Refuser le mot de passe public lors d'une écoute réseau. La liaison par défaut à `127.0.0.1` réduit l'exposition ; elle ne corrige pas la faiblesse lors du passage documenté à `0.0.0.0`.

## SEC-03 — Moyenne — Fuite des pions masqués dans l'ordre de combat

**Sources :** `src/main.rs:65–110` (snapshot), `src/main.rs:609–629` (combat_add), `src/scene_tools.rs:114` (projection de vision).

Les pions masqués ou soustraits de la projection de scène peuvent rester dans `world.combat.order`, transmis sans filtrage de visibilité. Ce document contient identifiant, nom et initiative.

**Preuve reproduite :** le MJ masque `wolf`, puis l'ajoute au combat ; un joueur ne reçoit pas ce pion dans `scenes[0].tokens`, mais reçoit `wolf`, « Loup des brumes » et son initiative dans `combat.order`. Cela divulgue une rencontre préparée ou un adversaire caché.

**Correction :** projeter le combat selon les mêmes règles de visibilité que les scènes ; retirer ou anonymiser les combattants cachés. Adapter l'index de tour projeté si des entrées sont retirées. Tester masque explicite, brouillard et vision limitée. Si afficher ces combattants est voulu, rendre la divulgation explicite dans l'interface et la documentation.

## SEC-04 — Moyenne — Absence de quotas et de cycle de vie des sessions

**Sources :** `src/main.rs:183–205` (connexion et insertion), `src/main.rs:31` (HashMap), `src/main.rs:668–680` (mutation sous verrou et écriture disque), `src/main.rs:683` (SSE).

Chaque connexion réussie crée une session conservée jusqu'au redémarrage. Aucun mécanisme d'expiration, de déconnexion/révocation ou plafond de sessions n'est présent dans le code. Aucune limitation des tentatives de mot de passe n'est définie. Un client peut accumuler des sessions et tenter des mots de passe ; un joueur peut également multiplier les mutations qui clonent le monde puis le sauvegardent sous le verrou global, ralentissant les autres clients.

**Observation limitée reproduite :** 25 mauvais mots de passe successifs donnent tous 401, puis 25 connexions joueur réussissent sans 429. Ce petit essai confirme l'absence de frein sur cette séquence ; aucun épuisement mémoire, saturation disque ou arrêt du service n'a été provoqué.

**Correction :** limitation de débit par origine réseau et utilisateur avec temporisation progressive, plafond global/par utilisateur, expiration serveur et révocation. Encadrer les connexions SSE et la fréquence des mutations ; limiter aussi la croissance persistante du monde. Un plafond de 20 brouillons par nom dans le constructeur est contournable avec de nouveaux noms.

## SEC-05 — Moyenne sur un réseau non fiable — Authentification transportée en HTTP

**Sources :** `src/main.rs:209` (cookie), `src/main.rs:750–756` (écoute TCP/serveur HTTP), instructions réseau du README.

Le serveur ne termine pas TLS et le mode réseau documenté utilise HTTP. Un acteur capable d'observer ce trafic peut récupérer le mot de passe MJ ou le cookie de session ; un acteur capable de le modifier peut altérer l'interface servie. Le cookie n'a pas l'attribut Secure. HttpOnly et SameSite=Strict sont bien présents, mais ne chiffrent pas le transport.

**Preuve locale :** connexion réussie via une URL HTTP ; Set-Cookie contient HttpOnly et SameSite=Strict, sans Secure. Aucun trafic de tiers n'a été intercepté.

**Correction :** pour les accès distants, terminer TLS sur un proxy correctement configuré, interdire l'accès direct au port HTTP et émettre des cookies Secure dans ce mode. Ne pas ajouter simplement Secure tout en conservant un parcours réseau HTTP inutilisable. Réserver le parcours HTTP au développement local maîtrisé.

## Autres points et limites

- Les textes utilisateur examinés passent principalement par `textContent`. Les usages `innerHTML` recensés servent aux icônes définies dans le code ; aucune XSS exploitable n'a été démontrée.
- Les routes d'assets reposent sur une sélection explicite de ressources intégrées ; aucune lecture arbitraire de chemin n'a été identifiée sur ce parcours.
- Les commandes de dés sont interprétées côté Rust ; aucun moteur d'exécution de code utilisateur identifié.
- Le corps HTTP est limité à 16 Mio ; ce plafond ne borne ni le nombre de sessions ni la taille totale accumulée du monde.
- La carte complète reste accessible malgré le brouillard, limite déjà signalée dans le README. Ne pas placer de secrets dans son image si les joueurs ne doivent pas les découvrir.
- Les réponses ne définissent pas de CSP ni de protection contre l'intégration en frame dans le routeur examiné. Durcissement à prévoir selon le déploiement ; aucun exploit distinct démontré.
- Les imports restent réservés au MJ. La validation utilise des valeurs JSON génériques et des `unwrap` : une revue plus poussée des formes importées est utile avant d'accepter des mondes provenant de tiers.

## Ordre de traitement recommandé

1. Remplacer l'identité fondée sur le nom et supprimer le secret MJ par défaut.
2. Corriger la projection du combat et transformer les reproductions en tests de protection.
3. Ajouter quotas, expiration/révocation et transport HTTPS pour l'accès réseau.
4. Exécuter un audit RustSec sur Cargo.lock et l'intégrer aux vérifications régulières.
