import { test, expect } from '../fixtures/table.js';

test('une réponse HTTP ancienne ne remplace pas un état plus récent', async ({ table }) => {
  const gm = await table.open();
  const snapshot = await gm.state();
  let releaseOld;
  const gate = new Promise((resolve) => {
    releaseOld = resolve;
  });
  let markIntercepted;
  const intercepted = new Promise((resolve) => {
    markIntercepted = resolve;
  });
  let oldRequest;
  await gm.page.route('**/api/state', async (route) => {
    if (oldRequest) {
      await route.continue();
      return;
    }
    oldRequest = route.request();
    markIntercepted();
    await gate;
    await route.fulfill({ json: snapshot });
  });
  try {
    // Trigger refresh through a genuine SSE event instead of replacing application state.
    const response = await gm.page.request.post('/api/action', {
      data: { kind: 'chat', text: 'Première révision' },
    });
    expect(response.ok()).toBeTruthy();
    await intercepted;
    await gm.chat('Dernière révision');
    await expect(gm.panel.getByText('Dernière révision', { exact: true })).toBeVisible();
    const delivered = gm.page.waitForResponse((response) => response.request() === oldRequest);
    releaseOld();
    await (await delivered).finished();
    await gm.page.evaluate(() => new Promise(requestAnimationFrame));
    // A later real action confirms rendering continues from the current world.
    await expect(gm.panel.getByText('Dernière révision', { exact: true })).toBeVisible();
    await expect(gm.panel.getByText('Première révision', { exact: true })).toBeVisible();
  } finally {
    releaseOld();
  }
});

test('formulaire au clavier : focus contenu, sauvegarde et fermeture par Échap', async ({
  table,
}) => {
  const gm = await table.open();
  await gm.tab('Personnages');
  const create = gm.page.getByTitle('Créer un document', { exact: true });
  await create.focus();
  await gm.page.keyboard.press('Enter');
  await expect(gm.modal).toBeVisible();
  const name = gm.modal.getByLabel('Nom', { exact: true });
  await expect(name).toBeFocused();
  await name.fill('Personnage clavier');
  // HTML modal dialogs must contain both forward and backward keyboard focus.
  for (let i = 0; i < 14; i++) {
    await gm.page.keyboard.press('Tab');
    await expect
      .poll(() => gm.modal.evaluate((dialog) => dialog.contains(document.activeElement)))
      .toBe(true);
  }
  await gm.modal.getByRole('button', { name: 'Enregistrer', exact: true }).focus();
  await gm.page.keyboard.press('Enter');
  await expect(gm.modal).not.toBeVisible();
  await expect(
    gm.panel.getByRole('heading', { name: 'Personnage clavier', exact: true }),
  ).toBeVisible();
  await create.focus();
  await gm.page.keyboard.press('Enter');
  await expect(gm.modal).toBeVisible();
  await gm.page.keyboard.press('Escape');
  await expect(gm.modal).not.toBeVisible();
  await expect(create).toBeFocused();
});
