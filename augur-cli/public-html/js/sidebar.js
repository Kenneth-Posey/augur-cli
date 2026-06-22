/**
 * sidebar.js - Sidebar panel management
 *
 * Renders the right-side detail panel for a leaf module, showing:
 * documentation, dependency edges (inbound/outbound), symbols
 * (functions, types, traits, etc.), and a link to API docs.
 * Also provides closeSidebar() and Escape-key shortcut wiring.
 */
function renderSidebar(moduleId, crateId) {
    var crateData = state.data.crates[crateId];
    if (!crateData) return;

    var nodeData = null;
    crateData.nodes.forEach(function (n) {
        if (n.id === moduleId) nodeData = n;
    });
    if (!nodeData) return;

    state.sidebarModule = moduleId;

    var outboundEdges = [];
    var inboundEdges = [];
    if (crateData.edges) {
        crateData.edges.forEach(function (e) {
            if (e.source === moduleId) outboundEdges.push(e.target);
            if (e.target === moduleId) inboundEdges.push(e.source);
        });
    }

    var cratePath = crateId.replace(/-/g, '_');
    var docPath = moduleId.replace(/^[^:]+::/, '').replace(/::/g, '/');
    if (docPath === 'lib' || docPath === 'main') {
        docPath = '';
    }
    var apiUrl = docPath
        ? 'api/' + cratePath + '/' + docPath + '/index.html'
        : 'api/' + cratePath + '/index.html';

    var html = '';
    html += '<button class="close-btn" id="sidebar-close">&times;</button>';
    html += '<h2>' + escapeHtml(nodeData.label) + '</h2>';
    html += '<div class="module-path">' + escapeHtml(moduleId) + '</div>';

    html += '<div class="section-label">Documentation</div>';
    if (nodeData.doc && nodeData.doc.trim()) {
        html += '<div class="doc-text">' + escapeHtml(nodeData.doc) + '</div>';
    } else {
        html += '<div class="doc-text doc-empty">No documentation comment found.</div>';
    }

    html += '<div class="section-label">Depends On (' + outboundEdges.length + ')</div>';
    if (outboundEdges.length > 0) {
        html += '<ul class="edge-list">';
        outboundEdges.forEach(function (target) {
            html += '<li>' + escapeHtml(target) + '</li>';
        });
        html += '</ul>';
    } else {
        html += '<div class="doc-text doc-empty">No intra-crate dependencies.</div>';
    }

    html += '<div class="section-label">Depended By (' + inboundEdges.length + ')</div>';
    if (inboundEdges.length > 0) {
        html += '<ul class="edge-list">';
        inboundEdges.forEach(function (source) {
            html += '<li class="inbound">' + escapeHtml(source) + '</li>';
        });
        html += '</ul>';
    } else {
        html += '<div class="doc-text doc-empty">No intra-crate dependents.</div>';
    }

    var symbols = nodeData.symbols || [];
    if (symbols.length > 0) {
        html += '<div class="section-label">Symbols (' + symbols.length + ')</div>';
        html += '<ul class="edge-list">';
        symbols.forEach(function (sym) {
            html += '<li>' + escapeHtml(sym) + '</li>';
        });
        html += '</ul>';
    }

    html += '<div><a class="api-link" href="' + escapeHtml(apiUrl) + '" target="_blank">View API Docs &rarr;</a></div>';

    $sidebar.innerHTML = html;
    $sidebar.classList.add('visible');

    document.getElementById('sidebar-close').addEventListener('click', closeSidebar);
}

function closeSidebar() {
    state.sidebarModule = null;
    $sidebar.classList.remove('visible');
}