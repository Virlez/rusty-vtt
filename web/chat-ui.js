'use strict';
(() => {
  const controls = document.querySelector('#chat-compose .chat-controls');
  if (!controls) return;
  const mode = document.createElement('select');
  mode.id = 'chat-visibility';
  mode.setAttribute('aria-label', 'Visibilité des messages et des jets');
  for (const [value, label] of [['public','Public'],['gm','MJ et moi'],['blind','Jet aveugle (MJ)'],['self','Moi uniquement'],['whisper','Murmure']]) {
    const option = document.createElement('option'); option.value = value; option.textContent = label; mode.append(option);
  }
  const recipients = document.createElement('input');
  recipients.id = 'chat-recipients'; recipients.placeholder = 'Noms exacts, séparés par une virgule';
  recipients.setAttribute('aria-label', 'Destinataires du murmure'); recipients.hidden = true;
  const oldModeLabel = controls.querySelector('.chat-mode');
  if (oldModeLabel) oldModeLabel.remove();
  const description = controls.querySelector('span:not([data-icon])');
  if (description) description.textContent = 'Visibilité';
  controls.append(mode); controls.after(recipients);
  mode.onchange = () => { recipients.hidden = mode.value !== 'whisper'; };
  const previousAction = action;
  action = async function(kind, data = {}) {
    if (kind === 'chat' || kind === 'roll') {
      data = {...data};
      if (!data.mode) data.mode = mode.value;
      if (data.mode === 'whisper' && !data.recipients) data.recipients = recipients.value.split(',').map(s => s.trim()).filter(Boolean);
    }
    return previousAction(kind, data);
  };
  document.querySelector('#chat-form').onsubmit = async event => {
    event.preventDefault();
    const input = document.querySelector('#message'), text = input.value.trim();
    if (!text) return;
    let kind = 'chat', data = {text};
    const command = text.match(/^\/(r|roll|gmroll|gmr|blindroll|br|selfroll|sr)\s+(.+)$/i);
    const whisper = text.match(/^\/w\s+(?:"([^"]+)"|(\S+))\s+([\s\S]+)$/i);
    if (command) {
      kind = 'roll'; data = {formula: command[2]};
      const cmd = command[1].toLowerCase();
      if (['gmroll','gmr'].includes(cmd)) data.mode = 'gm';
      if (['blindroll','br'].includes(cmd)) data.mode = 'blind';
      if (['selfroll','sr'].includes(cmd)) data.mode = 'self';
    } else if (whisper) data = {text: whisper[3], mode: 'whisper', recipients: [whisper[1] || whisper[2]]};
    const success = await action(kind, data);
    if (success && input.value.trim() === text) input.value = '';
  };
  const previousPanel = renderPanel;
  renderPanel = function() {
    previousPanel();
    if (!state || tab !== 'chat') return;
    const entries = document.querySelectorAll('#panel-body .entry');
    const labels = {gm:'MJ et auteur', blind:'Jet aveugle · MJ', self:'Personnel', whisper:'Murmure'};
    state.world.messages.forEach((message, index) => {
      if (!labels[message.mode] || !entries[index]) return;
      const label = document.createElement('small');
      label.textContent = labels[message.mode] + (message.mode === 'whisper' ? ' → ' + (message.recipients || []).join(', ') : '');
      entries[index].querySelector('.entry-header')?.append(label);
    });
  };
  document.querySelector('#message').placeholder = '/r 1d20+5 · /gmroll 1d20 · /blindroll 1d20 · /w "Nom" message';
  if (state) renderPanel();
})();
