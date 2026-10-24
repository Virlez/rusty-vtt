import { test, expect } from '../fixtures/table.js';

test.describe('Sessions et synchronisation', () => {
  test('connexion refusée, session conservée au rechargement et déconnexion', async ({ table, page }) => {
    await page.goto(table.url);
    await page.getByLabel('Votre nom configuré par le meneur').fill('Alice');
    await page.getByLabel('Votre secret personnel ou le secret du meneur').fill('incorrect');
    await page.getByRole('button', { name: 'Rejoindre l’aventure' }).click();
    await expect(page.locator('#modal-error')).not.toBeEmpty();
    await expect(page.getByRole('dialog')).toBeVisible();
    const app = await table.open('player');
    await app.page.reload();
    await expect(app.page.getByRole('button', { name: 'Alice · Joueur' })).toBeVisible();
    await app.page.getByRole('button', { name: 'Alice · Joueur' }).click();
    await expect(app.page.getByRole('button', { name: 'Rejoindre l’aventure' })).toBeVisible();
    expect((await app.page.request.get('/api/state')).status()).toBe(401);
  });

  test('chat et dés synchronisés entre deux sessions sans rechargement', async ({ table }) => {
    const gm = await table.open(), player = await table.open('player');
    await player.chat('Bonjour la table <script>');
    await expect(gm.panel.getByText('Bonjour la table <script>', { exact: true })).toBeVisible();
    await player.chat('/r 2d6+3');
    await expect(gm.panel.locator('.roll small').filter({ hasText: '2d6+3' })).toBeVisible();
    const message = (await gm.state()).world.messages.at(-1);
    expect(message.roll.total).toBeGreaterThanOrEqual(5);
    expect(message.roll.total).toBeLessThanOrEqual(15);
    await player.chat('/r 101d6');
    await expect(player.page.getByRole('status')).not.toBeEmpty();
    expect((await gm.state()).world.messages).toHaveLength(2);
  });
});

test.describe('Documents et rencontre', () => {
  test('création, édition par le propriétaire, recherche et initiative', async ({ table }) => {
    const gm = await table.open(), player = await table.open('player');
    await gm.tab('Personnages');
    await gm.create({ Nom: 'Sentinelle E2E', 'Nom du joueur propriétaire': 'Alice', 'Points de vie': 25 });
    await player.tab('Personnages');
    const card = player.panel.locator('.card').filter({ has: player.page.getByRole('heading', { name: 'Sentinelle E2E', exact: true }) });
    await card.getByRole('button', { name: 'Ouvrir la fiche' }).click();
    await player.modal.getByLabel('Points de vie', { exact: true }).fill('17');
    await player.save();
    await expect(gm.panel.locator('.card').filter({ has: gm.page.getByRole('heading', { name: 'Sentinelle E2E', exact: true }) })).toContainText('17 / 20 PV');
    await gm.page.getByRole('searchbox').fill('absent');
    await expect(gm.panel.getByRole('heading', { name: 'Sentinelle E2E' })).not.toBeVisible();
    await gm.page.getByRole('searchbox').fill('Sentinelle');
    await gm.panel.getByRole('button', { name: '⚔ Initiative' }).click();
    await gm.page.getByRole('searchbox').clear();
    await gm.tab('Rencontre');
    await expect(gm.panel).toContainText('Sentinelle E2E');
    await gm.page.getByRole('button', { name: 'Tour suivant →' }).click();
    await expect(gm.panel).toContainText('Round 2');
  });

  test('journal public et privé, objets et ambiance', async ({ table }) => {
    const gm = await table.open(), player = await table.open('player');
    await gm.tab('Journal');
    await gm.create({ Titre: 'Récit public', 'Votre récit': 'La quête commence.' });
    await gm.create({ Titre: 'Secret E2E', 'Votre récit': 'Le trésor caché', 'Visible par les joueurs': false });
    await player.tab('Journal');
    await expect(player.panel).toContainText('Récit public');
    await expect(player.panel).not.toContainText('Secret E2E');
    expect(JSON.stringify((await player.state()).world.journals)).not.toContain('Le trésor caché');
    await gm.tab('Objets');
    await gm.create({ Nom: 'Épée E2E', Description: 'Une lame ancienne' });
    await player.tab('Objets');
    await expect(player.panel).toContainText('Une lame ancienne');
    await gm.tab('Ambiance');
    await gm.create({ Titre: 'Forêt E2E', 'URL du fichier audio': 'https://example.com/forest.ogg' });
    await expect(gm.panel.locator('audio').last()).toHaveAttribute('src', 'https://example.com/forest.ogg');
  });
});

test.describe('Scènes, sauvegarde et modules', () => {
  test('création sur le canvas, murs, brouillard et dissipation', async ({ table }) => {
    const gm = await table.open();
    const canvas = gm.page.getByLabel('Carte de la scène');
    await gm.page.getByRole('button', { name: 'Ajouter un pion', exact: true }).click();
    await canvas.click({ position: { x: 350, y: 300 } });
    await gm.modal.getByLabel('Nom', { exact: true }).fill('Pion E2E');
    await gm.save();
    await expect.poll(async () => (await gm.state()).world.scenes[0].tokens.some(t => t.name === 'Pion E2E')).toBe(true);
    const box = await canvas.boundingBox();
    for (const tool of ['Contrôles des murs', 'Brouillard de guerre']) {
      await gm.page.getByRole('button', { name: tool, exact: true }).click();
      await gm.page.mouse.move(box.x + 250, box.y + 250);
      await gm.page.mouse.down();
      await gm.page.mouse.move(box.x + 450, box.y + 400, { steps: 5 });
      await gm.page.mouse.up();
      const key = tool === 'Contrôles des murs' ? 'walls' : 'fog';
      await expect.poll(async () => (await gm.state()).world.scenes[0][key].length).toBe(1);
    }
    await gm.tab('Paramètres');
    await gm.page.getByRole('button', { name: 'Dissiper tout le brouillard' }).click();
    await expect.poll(async () => (await gm.state()).world.scenes[0].fog.length).toBe(0);
  });

  test('raccourci personnalisé conservé après rechargement et lancé au clavier', async ({ table }) => {
    const gm = await table.open();
    // The slots have dynamically composed names; scope the stable hotbar container.
    await gm.page.locator('#macro-slots button').first().click({ button: 'right' });
    await gm.modal.getByLabel('Formule de dés (vide pour effacer)').fill('2d6+4');
    await gm.save();
    await gm.page.reload();
    await expect(gm.page.locator('#macro-slots button').first()).toContainText('2d6+4');
    await expect(gm.page.locator('#identity')).toContainText('Meneur');
    await gm.page.getByLabel('Carte de la scène').click({ position: { x: 300, y: 300 } });
    await gm.page.keyboard.press('1');
    await expect(gm.panel.locator('.roll small')).toHaveText('2d6+4');
  });

  test('jets aveugles et personnels filtrés entre navigateur joueur et MJ', async ({ table }) => {
    const gm = await table.open(), player = await table.open('player');
    await player.chat('/blindroll 1d20');
    await expect(gm.panel.locator('.roll')).toHaveCount(1);
    expect((await player.state()).world.messages).toHaveLength(0);
    await player.chat('/selfroll 1d6');
    await expect(player.panel.locator('.roll')).toHaveCount(1);
    expect((await gm.state()).world.messages).toHaveLength(1);
    await gm.chat('/w "Alice" Rendez-vous secret');
    await expect(player.panel.getByText('Rendez-vous secret', { exact: true })).toBeVisible();
  });

  test('création et activation de scène propagées au joueur', async ({ table }) => {
    const gm = await table.open(), player = await table.open('player');
    await gm.page.getByTitle('Créer une scène', { exact: true }).click();
    await gm.modal.getByLabel('Nom du lieu').fill('Ruines E2E');
    await gm.save();
    await gm.page.locator('#scenes').getByRole('button', { name: 'Ruines E2E' }).click();
    await expect(player.page.locator('#scene-title')).toHaveText('Ruines E2E');
    await player.tab('Paramètres');
    await expect(player.page.getByRole('link', { name: '↓ Exporter le monde' })).toHaveCount(0);
    await expect(player.page.getByRole('button', { name: 'Gérer les modules' })).toHaveCount(0);
  });

  test('export téléchargé et restauration par import dans le navigateur', async ({ table }) => {
    const gm = await table.open();
    await gm.chat('Sauvegarde E2E');
    await gm.tab('Paramètres');
    const downloadPromise = gm.page.waitForEvent('download');
    await gm.page.getByRole('link', { name: '↓ Exporter le monde' }).click();
    const download = await downloadPromise;
    expect(download.suggestedFilename()).toMatch(/\.json$/);
    await gm.chat('Après export');
    await gm.tab('Paramètres');
    const chooserPromise = gm.page.waitForEvent('filechooser');
    await gm.page.getByRole('button', { name: '↑ Importer un monde' }).click();
    await (await chooserPromise).setFiles(await download.path());
    await expect.poll(async () => (await gm.state()).world.messages.map(m => m.text)).not.toContain('Après export');
    await gm.tab('Conversation');
    await expect(gm.panel).toContainText('Sauvegarde E2E');
  });

  test('activation du module et reprise du brouillon 5e', async ({ table }) => {
    const gm = await table.open();
    await gm.tab('Paramètres');
    await gm.page.getByRole('button', { name: 'Gérer les modules' }).click();
    const manager = gm.page.locator('#module-manager');
    await manager.getByRole('checkbox').check();
    await manager.getByRole('button', { name: 'Ouvrir le créateur' }).click();
    const builder = gm.page.locator('#character-builder-fr');
    await expect(builder).toBeVisible();
    await builder.getByRole('button', { name: 'Fermer et conserver' }).click();
    await gm.tab('Personnages');
    await gm.page.getByRole('button', { name: /^Reprendre :/ }).click();
    await expect(builder).toBeVisible();
    expect((await gm.state()).world.builder_drafts).toHaveLength(1);
  });
});
