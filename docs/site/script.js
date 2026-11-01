// Lux Landing Page Interactivity

document.addEventListener('DOMContentLoaded', () => {
  // Keyboard Shortcuts Tabs
  const shortcutData = [
    { name: "Goto Anything", mac: ["Cmd", "P"], win: ["Ctrl", "P"] },
    { name: "Command Palette", mac: ["Cmd", "Shift", "P"], win: ["Ctrl", "Shift", "P"] },
    { name: "Add Next Occurrence", mac: ["Cmd", "D"], win: ["Ctrl", "D"] },
    { name: "Skip Occurrence", mac: ["Cmd", "K", "Cmd", "D"], win: ["Ctrl", "K", "Ctrl", "D"] },
    { name: "Select All Occurrences", mac: ["Ctrl", "Cmd", "G"], win: ["Alt", "F3"] },
    { name: "Split Selection into Lines", mac: ["Cmd", "Shift", "L"], win: ["Ctrl", "Shift", "L"] },
    { name: "Column Selection", mac: ["Option", "Drag"], win: ["Alt", "Drag"] },
    { name: "Layout 1 / 2 / 3 / 4 / Grid", mac: ["Cmd", "Option", "1..5"], win: ["Alt", "Shift", "1..5"] },
    { name: "Focus Pane Group", mac: ["Ctrl", "1..4"], win: ["Ctrl", "1..4"] },
    { name: "Move Tab to Group", mac: ["Ctrl", "Shift", "1..4"], win: ["Ctrl", "Shift", "1..4"] },
    { name: "Fold / Unfold Code", mac: ["Cmd", "Option", "[ / ]"], win: ["Ctrl", "Shift", "[ / ]"] },
    { name: "Toggle Terminal PTY", mac: ["Cmd", "`"], win: ["Ctrl", "`"] }
  ];

  const shortcutsGrid = document.getElementById('shortcuts-grid');
  const shortcutTabs = document.querySelectorAll('.shortcut-tab');
  let currentOS = 'mac';

  function renderShortcuts(os) {
    if (!shortcutsGrid) return;
    shortcutsGrid.innerHTML = '';
    shortcutData.forEach(item => {
      const card = document.createElement('div');
      card.className = 'shortcut-card';

      const name = document.createElement('span');
      name.className = 'shortcut-name';
      name.textContent = item.name;

      const keys = document.createElement('div');
      keys.className = 'shortcut-keys';

      const keyList = os === 'mac' ? item.mac : item.win;
      keyList.forEach(k => {
        const kbd = document.createElement('kbd');
        kbd.textContent = k;
        keys.appendChild(kbd);
      });

      card.appendChild(name);
      card.appendChild(keys);
      shortcutsGrid.appendChild(card);
    });
  }

  shortcutTabs.forEach(tab => {
    tab.addEventListener('click', () => {
      shortcutTabs.forEach(t => t.classList.remove('active'));
      tab.classList.add('active');
      currentOS = tab.getAttribute('data-os');
      renderShortcuts(currentOS);
    });
  });

  renderShortcuts(currentOS);

  // 5. Copy Build Command
  const copyBtn = document.getElementById('copy-btn');
  const copyText = document.getElementById('copy-text');
  if (copyBtn) {
    copyBtn.addEventListener('click', () => {
      const cmd = "git clone https://github.com/lcajigasm/lux.git\ncd lux && cargo build --release";
      navigator.clipboard.writeText(cmd).then(() => {
        copyText.textContent = "Copied!";
        copyBtn.style.background = "rgba(86, 182, 194, 0.2)";
        setTimeout(() => {
          copyText.textContent = "Copy";
          copyBtn.style.background = "";
        }, 2000);
      });
    });
  }
});
