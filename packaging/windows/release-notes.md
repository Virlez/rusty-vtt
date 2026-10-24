Version **alpha** de Rusty VTT pour Windows 10/11 x64.

1. Téléchargez **Rusty-VTT-…-windows-x64-setup.exe** dans les fichiers ci-dessous.
2. Installez puis lancez **Rusty VTT Alpha** depuis le menu Démarrer.
3. Le navigateur ouvre la table. Utilisez le secret MJ affiché dans la console pour vous connecter.
4. Gardez cette console ouverte ; **Ctrl+C** arrête le serveur.

Aucune installation de Rust ou Node.js nécessaire. Installation pour l'utilisateur courant sans droits administrateur. Le programme alpha n'est pas signé numériquement ; Windows peut afficher un avertissement SmartScreen.

Sauvegardes : `%LOCALAPPDATA%\Rusty VTT\data\world.json`. Les mises à jour et la désinstallation conservent les données. Exportez régulièrement vos mondes : cette alpha peut contenir des bugs et son format peut évoluer.

Cette installation démarre une table locale. Pour accueillir des joueurs à distance, configurez HTTPS et les secrets individuels comme indiqué dans le README. Le secret MJ généré automatiquement change à chaque démarrage.

`SHA256SUMS.txt` contient les empreintes des installateurs. macOS et Linux : compilation depuis les sources pour le moment.
