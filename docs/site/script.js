// Lux Landing Page Interactivity

document.addEventListener('DOMContentLoaded', () => {
  // 1. Interactive Mockup Split Layout Switcher
  const splitBtns = document.querySelectorAll('.split-btn');
  const editorBody = document.getElementById('editor-body');

  splitBtns.forEach(btn => {
    btn.addEventListener('click', () => {
      splitBtns.forEach(b => b.classList.remove('active'));
      btn.classList.add('active');

      const splitType = btn.getAttribute('data-split');
      if (splitType === '1') {
        editorBody.className = 'editor-body split-mode-1';
      } else {
        editorBody.className = 'editor-body split-mode-2';
      }
    });
  });

  // 2. Mockup Tab Switcher
  const tabs = document.querySelectorAll('.editor-tabs .tab');
  const codeAreaLeft = document.querySelector('#pane-left .code-area pre code');
  const gutterLeft = document.querySelector('#pane-left .gutter');

  const fileContents = {
    tab1: {
      name: 'crates/lux/src/main.rs',
      lines: 15,
      code: `<span class="syn-kw">pub async fn</span> <span class="syn-fn">run_desktop_app</span>(cfg: <span class="syn-type">EditorConfig</span>) -> <span class="syn-type">Result</span>&lt;()&gt; {
    <span class="syn-cm">// Pure WGPU 120Hz native renderer - zero web-view overhead</span>
    <span class="syn-kw">let</span> renderer = <span class="syn-type">WgpuRenderer</span>::<span class="syn-fn">new</span>(&amp;window).<span class="syn-kw">await</span>?;
    <span class="syn-kw">let mut</span> session = <span class="syn-type">Session</span>::<span class="syn-fn">restore_hot_exit</span>()?;

    <span class="syn-cm">// Independent multi-pane layout architecture</span>
    <span class="syn-kw">let mut</span> split = <span class="syn-type">SplitLayout</span>::<span class="syn-fn">new</span>(<span class="syn-type">LayoutType</span>::<span class="syn-fn">Columns2</span>);
    split.<span class="syn-fn">attach_buffer</span>(session.<span class="syn-fn">active_buffer_id</span>());

    <span class="syn-cm">// Real-time tree-sitter incremental syntax typing</span>
    <span class="syn-kw">let</span> syntax = <span class="syn-type">BackgroundHighlighter</span>::<span class="syn-fn">spawn_worker</span>();
    event_loop.<span class="syn-fn">run</span>(<span class="syn-kw">move</span> |event, target| {
        session.<span class="syn-fn">dispatch_sublime_input</span>(&amp;event, &amp;target);<span class="cursor blinking"></span>
    })
}`
    },
    tab2: {
      name: 'crates/lux/src/split.rs',
      lines: 12,
      code: `<span class="syn-cm">/// Real multi-pane layout controller with draggable dividers</span>
<span class="syn-kw">pub struct</span> <span class="syn-type">SplitLayout</span> {
    <span class="syn-kw">pub</span> panes: <span class="syn-type">Vec</span>&lt;<span class="syn-type">PaneView</span>&gt;,
    <span class="syn-kw">pub</span> ratio: <span class="syn-type">f32</span>, <span class="syn-cm">// Clamped [0.1..0.9]</span>
    <span class="syn-kw">pub</span> layout_type: <span class="syn-type">LayoutType</span>,
}

<span class="syn-kw">impl</span> <span class="syn-type">SplitLayout</span> {
    <span class="syn-kw">pub fn</span> <span class="syn-fn">handle_divider_drag</span>(&amp;<span class="syn-kw">mut</span> <span class="syn-type">self</span>, x: <span class="syn-type">f32</span>, width: <span class="syn-type">f32</span>) {
        <span class="syn-type">self</span>.ratio = (x / width).<span class="syn-fn">clamp</span>(<span class="syn-num">0.1</span>, <span class="syn-num">0.9</span>);<span class="cursor blinking"></span>
    }
}`
    },
    tab3: {
      name: 'crates/lux-config/settings.jsonc',
      lines: 10,
      code: `{
    <span class="syn-type">"theme"</span>: <span class="syn-fn">"Mariana"</span>,
    <span class="syn-type">"font_size"</span>: <span class="syn-num">14.0</span>,
    <span class="syn-type">"font_family"</span>: <span class="syn-fn">"Fira Code"</span>,
    <span class="syn-type">"tab_size"</span>: <span class="syn-num">4</span>,
    <span class="syn-type">"hardware_acceleration"</span>: <span class="syn-kw">true</span>,
    <span class="syn-type">"telemetry"</span>: <span class="syn-kw">false</span>,<span class="cursor blinking"></span>
    <span class="syn-type">"hot_exit"</span>: <span class="syn-fn">"always"</span>
}`
    }
  };

  tabs.forEach(tab => {
    tab.addEventListener('click', () => {
      tabs.forEach(t => t.classList.remove('active'));
      tab.classList.add('active');

      const tabKey = tab.getAttribute('data-tab');
      const data = fileContents[tabKey];
      if (data) {
        document.querySelector('#pane-left .pane-title').textContent = data.name;
        codeAreaLeft.innerHTML = data.code;
        
        // Update gutter numbers
        let gutterHtml = '';
        for (let i = 1; i <= data.lines; i++) {
          gutterHtml += `<span>${i}</span>`;
        }
        gutterLeft.innerHTML = gutterHtml;
      }
    });
  });

  // 3. Interactive Split Divider Dragging (Mockup)
  const divider = document.getElementById('editor-divider');
  const leftPane = document.getElementById('pane-left');
  let isDragging = false;

  if (divider) {
    divider.addEventListener('mousedown', (e) => {
      isDragging = true;
      document.body.style.cursor = 'col-resize';
      e.preventDefault();
    });

    window.addEventListener('mousemove', (e) => {
      if (!isDragging) return;
      const rect = editorBody.getBoundingClientRect();
      const offset = e.clientX - rect.left;
      const ratio = offset / rect.width;
      if (ratio >= 0.2 && ratio <= 0.8) {
        leftPane.style.flex = 'none';
        leftPane.style.width = `${ratio * 100}%`;
      }
    });

    window.addEventListener('mouseup', () => {
      if (isDragging) {
        isDragging = false;
        document.body.style.cursor = 'default';
      }
    });

    // Double click to reset 50/50
    divider.addEventListener('dblclick', () => {
      leftPane.style.flex = '1';
      leftPane.style.width = 'auto';
    });
  }

  // 4. Keyboard Shortcuts Tabs
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
