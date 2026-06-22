/**
 * loader.js - Graph data loading and application bootstrap
 *
 * Fetches graph-data.json, validates its shape, initializes Cytoscape,
 * and starts at the workspace-level view. Also wires back button and
 * keyboard shortcuts (Escape to close sidebar, Backspace/Left to go back).
 */
function loadData() {
    showStatus('loading', '&#8987;', 'Loading graph data...');

    fetch('graph-data.json')
        .then(function (res) {
            if (!res.ok) {
                throw new Error('HTTP ' + res.status + ' ' + res.statusText);
            }
            return res.json();
        })
        .then(function (json) {
            if (!json.workspace || !json.crates) {
                throw new Error('Invalid graph data: missing workspace or crates section');
            }
            if (!json.workspace.nodes || json.workspace.nodes.length === 0) {
                showStatus('empty', '&#128196;', 'No workspace nodes found. The graph data is empty.');
                return;
            }

            state.data = json;
            hideStatus();

            if (!state.cy) {
                initCy();
            }

            navigateTo('workspace', '__root__');
        })
        .catch(function (err) {
            showStatus('error', '&#9888;', 'Failed to load graph-data.json:<br>' + escapeHtml(err.message));
            console.error('Graph data load error:', err);
        });
}

/* ---- Wire up back button ---- */
$backBtn.addEventListener('click', navigateBack);

/* ---- Keyboard shortcuts ---- */
document.addEventListener('keydown', function (e) {
    if (e.key === 'Escape' && state.sidebarModule !== null) {
        closeSidebar();
    }
});

document.addEventListener('keydown', function (e) {
    if ((e.key === 'Backspace' || e.key === 'ArrowLeft') &&
        state.stack.length > 1 &&
        state.sidebarModule === null &&
        !e.target.matches('input, textarea')) {
        e.preventDefault();
        navigateBack();
    }
});

/* ---- Start ---- */
loadData();