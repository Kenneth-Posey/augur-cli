/**
 * helpers.js - Shared utility functions
 *
 * Provides escapeHtml, dashedId, layer color helpers, status display,
 * breadcrumb rendering, back button updates, and the layout config factory.
 */

/* ---- Layer color palette ---- */
var LAYER_COLORS = [
    '#4ecdc4',  // layer 0 - foundation
    '#45b7d1',
    '#3d8ec0',
    '#2d6a9f',
    '#1e4a7a',
    '#16325b',
    '#0f2248',
];

function getLayerColor(layer) {
    var idx = Math.min(layer, LAYER_COLORS.length - 1);
    var c = LAYER_COLORS[idx];
    return {
        bg: c,
        border: idx === 0 ? '#6ef5ec' : c,
    };
}

/* ---- Status display ---- */
function showStatus(type, icon, text) {
    $status.className = type;
    $status.innerHTML = '<div class="status-icon">' + icon + '</div><div class="status-text">' + text + '</div>';
    $status.classList.remove('hidden');
}
function hideStatus() {
    $status.classList.add('hidden');
}

/* ---- HTML escaping ---- */
function escapeHtml(str) {
    if (!str) return '';
    var div = document.createElement('div');
    div.appendChild(document.createTextNode(str));
    return div.innerHTML;
}

/* ---- Ghost node ID helper ---- */
function dashedId(prefix, suffix) {
    return prefix + '::ghost::' + suffix;
}

/* ---- Breadcrumb helpers ---- */
function labelForStackEntry(entry) {
    if (entry.level === 'workspace') return 'workspace';
    if (entry.level === 'submodule') {
        var parts = entry.id.split('::');
        return parts[parts.length - 1] || entry.id;
    }
    var parts = entry.id.split('::');
    return parts[0] || entry.id;
}

function renderBreadcrumb() {
    var parts = [];
    state.stack.forEach(function (entry, i) {
        if (i > 0) {
            parts.push('<span class="crumb-sep">&rsaquo;</span>');
        }
        var label = labelForStackEntry(entry);
        var cls = (i === state.stack.length - 1) ? 'crumb-segment active' : 'crumb-segment';
        parts.push('<span class="' + cls + '" data-idx="' + i + '">' + escapeHtml(label) + '</span>');
    });
    $breadcrumb.innerHTML = parts.join('');

    $breadcrumb.querySelectorAll('.crumb-segment:not(.active)').forEach(function (el) {
        el.addEventListener('click', function () {
            var idx = parseInt(el.getAttribute('data-idx'), 10);
            while (state.stack.length > idx + 1) {
                state.stack.pop();
            }
            var top = state.stack[state.stack.length - 1];
            renderGraph(top.level, top.id);
            renderBreadcrumb();
            updateBackBtn();
        });
    });
}

function updateBackBtn() {
    $backBtn.disabled = state.stack.length <= 1;
}

/* ---- Layout config factory (used only when Cytoscape built-in layout needed) ---- */
function getLayoutConfig(level) {
    if (level === 'crate') {
        return {
            name: 'preset',
            positions: undefined,
            animate: false,
            fit: false,
        };
    }
    return {
        name: 'dagre',
        rankDir: 'TB',
        nodeSep: 60,
        rankSep: 80,
        padding: 40,
        animate: false,
        fit: true,
    };
}