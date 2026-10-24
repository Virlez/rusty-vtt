# Créateur de personnage 5e en français

Activation : **Paramètres → Gérer les modules → Créateur de personnage 5e — Français**. Ouvrir ensuite le créateur depuis ce gestionnaire ou le panneau Personnages.

Cette adaptation native Rust reprend le parcours de création du module [DnD 5e Character Builder](https://github.com/hammer-PvP/DnD-5e-Character-Builder), version 0.9.921, révision `1d406d374eb20d8fbadd4f666f4389443a60be0e`. Le code Foundry et libWrapper ne sont pas exécutés par Rusty.

## Utilisation

Les sept étapes couvrent identité, caractéristiques et historique, espèce, classe et maîtrises, sorts, équipement et vérification. Les brouillons se sauvegardent automatiquement et peuvent être repris. La validation serveur contrôle les choix avant de créer la fiche. Les joueurs accèdent à leurs brouillons et personnages ; le MJ peut configurer la création, les valeurs manuelles, le multiclassage, l’or supplémentaire et l’autorisation de progression.

Le catalogue français comprend les 12 classes, 9 espèces, 4 historiques et 339 sorts du SRD 5.2.1. Les caractéristiques acceptent tableau standard, achat de points ou tirage serveur verrouillé. La fiche propose jets, progression et repos court/long. Les sorts et pages de référence sont consultables en français.

## Couverture et limites

Il s’agit d’une adaptation du parcours, pas d’une reproduction complète des automatisations Foundry. Les règles sont celles du SRD moderne (2024), avec une sous-classe de référence par classe et une boutique de départ limitée à 20 articles. Les contenus supplémentaires des ouvrages commerciaux ne sont pas inclus. Les effets de nombreuses aptitudes, les actions de combat, la dépense des emplacements de sorts, les dons épiques et les intégrations de modules Foundry restent manuels. Les ressources de classe ne sont pas toutes réinitialisées automatiquement par les repos.

Les progressions et emplacements sont extraits des tables françaises sur 20 niveaux. Le multiclassage est optionnel et contrôle les prérequis ; les interactions avancées d’aptitudes ne sont pas automatisées.

## Sources et reproduction

- Source amont : `vendor/dnd5e-character-builder`, licence MIT conservée dans `modules/dnd5e-character-builder/LICENSE`.
- Règles : [SRD officiel](https://www.dndbeyond.com/srd), PDF français conservé dans `vendor/srd-fr/SRD-5.2.1-fr.pdf`.
- Extraction : `scripts/extract-srd-fr.py`, puis `node scripts/prepare-builder.mjs`.
- Validation : tests Rust et `node tests/builder-smoke.mjs` (serveur isolé, données temporaires).

Cette œuvre inclut du matériel issu du System Reference Document 5.2.1 (« SRD 5.2.1 ») de Wizards of the Coast LLC, disponible à l’adresse https://www.dndbeyond.com/srd. Le SRD 5.2.1 est régi par la Licence Creative Commons Attribution 4.0 International, disponible à l’adresse https://creativecommons.org/licenses/by/4.0/legalcode.
