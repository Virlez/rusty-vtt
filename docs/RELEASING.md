# Publier une alpha Windows

Le workflow `.github/workflows/release.yml` compile le serveur et l’installateur
Inno Setup sur Windows, exécute les tests Rust et HTTP, puis vérifie une installation
et une désinstallation silencieuses. Il publie l’installateur et ses empreintes
SHA-256 dans une **préversion GitHub** uniquement si ces étapes réussissent.

Depuis un commit poussé contenant les changements à distribuer :

```powershell
# Le préfixe numérique doit correspondre à la version de Cargo.toml.
git tag v0.1.0-alpha.1
git push origin v0.1.0-alpha.1
```

Utiliser un nouveau numéro alpha pour chaque publication. Le workflow refuse un
tag qui ne correspond pas à Cargo.toml. La publication nécessite les permissions
d’écriture du `GITHUB_TOKEN` sur le contenu du dépôt ; aucun secret supplémentaire
n’est nécessaire. Le lancement manuel du workflow produit seulement un artefact
de test téléchargeable dans Actions, sans créer de release.

Pour compiler localement avec Inno Setup 6 installé :

```powershell
cargo build --release --locked
& "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe" /DAppVersion=0.1.0-alpha.1 packaging/windows/installer.iss
```

Résultat : `dist/Rusty-VTT-0.1.0-alpha.1-windows-x64-setup.exe`.
L’interface et le catalogue sont intégrés au binaire ; aucun dossier de sauvegarde,
secret, environnement de développement ou fichier personnel n’est embarqué.

L’installateur est pour Windows 10/11 x64, par utilisateur, sans élévation.
Son identifiant reste stable entre versions pour permettre les mises à jour.
Le lanceur conserve les variables `RUSTY_*` déjà configurées et utilise sinon
`%LOCALAPPDATA%\Rusty VTT\data\world.json` et `127.0.0.1:3000`.
Il ne lance pas de navigateur en mode proxy HTTPS : utiliser alors l’URL du proxy.
Fermer le serveur avant une mise à jour. La désinstallation ne supprime jamais
le répertoire des campagnes. La signature de code n’est pas configurée pour l’alpha.
