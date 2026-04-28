import { listen } from '@tauri-apps/api/event';

const root = document.getElementById('root');

if (!root) {
    throw new Error('Probe window root element not found');
}

root.innerHTML = `
    <div style="padding: 14px; font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;">
        Waiting for probe data...
    </div>
`;

void listen<string>('probe-window:update-html', (event) => {
    root.innerHTML = event.payload;
});
