import { expect } from '@playwright/test';

export class TablePage {
  constructor(page) {
    this.page = page;
    // Stable IDs scope reusable windows and dynamic document containers.
    this.modal = page.locator('#modal');
    this.panel = page.locator('#panel-body');
  }
  async login(role = 'gm') {
    await this.page.getByLabel('Votre nom configuré par le meneur').fill(role === 'gm' ? 'MJ' : 'Alice');
    await this.page.getByLabel('Votre secret personnel ou le secret du meneur').fill(role === 'gm' ? 'e2e-gm-secret-123456' : 'e2e-alice-secret-123456');
    await this.page.getByRole('button', { name: 'Rejoindre l’aventure' }).click();
    await expect(this.page.getByRole('button', { name: role === 'gm' ? 'MJ · Meneur' : 'Alice · Joueur' })).toBeVisible();
    await expect(this.modal).not.toBeVisible();
  }
  async tab(name) { await this.page.getByRole('button', { name, exact: true }).click(); }
  async create(fields) {
    await this.page.getByTitle('Créer un document', { exact: true }).click();
    for (const [label, value] of Object.entries(fields)) {
      const input = this.modal.getByLabel(label, { exact: true });
      if (typeof value === 'boolean') await input.setChecked(value);
      else await input.fill(String(value));
    }
    await this.save();
  }
  async save() {
    await this.modal.getByRole('button', { name: 'Enregistrer', exact: true }).click();
    await expect(this.modal).not.toBeVisible();
  }
  async chat(text) {
    await this.tab('Conversation');
    await this.page.getByRole('textbox', { name: 'Message', exact: true }).fill(text);
    await this.page.getByRole('button', { name: 'Envoyer le message' }).click();
  }
  async state() {
    const response = await this.page.request.get('/api/state');
    expect(response.ok()).toBeTruthy();
    return response.json();
  }
}
