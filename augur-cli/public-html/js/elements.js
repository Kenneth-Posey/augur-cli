/**
 * elements.js - Cytoscape element builders
 *
 * Functions that produce Cytoscape element arrays from the graph data.
 * Each `build*` function constructs nodes and edges for a specific
 * navigation level: workspace, crate, and submodule drill-down.
 * Ghost nodes and cross-crate edges are added post-layout by
 * addCrossCrateElements.
 */

/* ---- buildElements dispatcher ---- */
function buildElements(level, id) {
    var elems = [];
    if (level === 'workspace') {
        return buildWorkspaceElements();
    } else if (level === 'crate') {
        return buildCrateElements(id);
    }
    return elems;
}

/* ---- Level 0: Workspace ---- */
function buildWorkspaceElements() {
    var elems = [];
    var ws = state.data.workspace;
    if (!ws || !ws.nodes) return elems;

    ws.nodes.forEach(function (n) {
        var colors = getLayerColor(n.layer || 0);
        elems.push({
            group: 'nodes',
            data: {
                id: n.id,
                label: n.label,
                level: '0',
                layer: n.layer || 0,
                doc: n.doc || '',
            },
            style: {
                'background-color': colors.bg,
                'border-color': colors.border,
            }
        });
    });

    if (ws.edges) {
        ws.edges.forEach(function (e) {
            elems.push({
                group: 'edges',
                data: {
                    id: 'we-' + e.source + '-' + e.target,
                    source: e.source,
                    target: e.target,
                }
            });
        });
    }

    return elems;
}

/* ---- Level 1: Crate ---- */
function buildCrateElements(crateId) {
    var elems = [];
    var crateData = state.data.crates[crateId];
    if (!crateData || !crateData.nodes) return elems;

    crateData.nodes.forEach(function (n) {
        var hasKids = (n.children || []).length > 0;
        var label = hasKids ? n.label + ' [+]' : n.label;
        elems.push({
            group: 'nodes',
            data: {
                id: n.id,
                label: label,
                level: '1',
                crate: crateId,
                doc: n.doc || '',
                visibility: n.visibility || 'pub',
                children: n.children || [],
                symbols: n.symbols || [],
                hasChildren: hasKids,
            },
            style: {
                'background-color': '#0f3460',
                'border-color': hasKids ? '#4ecdc4' : '#1a4a8a',
                'border-width': hasKids ? 2 : 1,
            }
        });
    });

    if (crateData.edges) {
        crateData.edges.forEach(function (e) {
            elems.push({
                group: 'edges',
                data: {
                    id: 'ie-' + e.source + '-' + e.target,
                    source: e.source,
                    target: e.target,
                }
            });
        });
    }

    return elems;
}

/* ---- Level 1.5: Submodule drill-down ---- */
function buildSubmoduleElements(crateId, parentModuleId) {
    var elems = [];
    var crateData = state.data.crates[crateId];
    if (!crateData || !crateData.nodes) return elems;

    var parentNode = null;
    for (var i = 0; i < crateData.nodes.length; i++) {
        if (crateData.nodes[i].id === parentModuleId) {
            parentNode = crateData.nodes[i];
            break;
        }
    }
    if (!parentNode) return elems;

    var childIdSet = {};
    (parentNode.children || []).forEach(function (cid) { childIdSet[cid] = true; });

    crateData.nodes.forEach(function (n) {
        if (!childIdSet[n.id]) return;
        var hasKids = (n.children || []).length > 0;
        var label = hasKids ? n.label + ' [+]' : n.label;
        elems.push({
            group: 'nodes',
            data: {
                id: n.id,
                label: label,
                level: '1',
                crate: crateId,
                doc: n.doc || '',
                visibility: n.visibility || 'pub',
                children: n.children || [],
                symbols: n.symbols || [],
                hasChildren: hasKids,
            },
            style: {
                'background-color': '#0f3460',
                'border-color': hasKids ? '#4ecdc4' : '#1a4a8a',
                'border-width': hasKids ? 2 : 1,
            }
        });
    });

    if (crateData.edges) {
        crateData.edges.forEach(function (e) {
            if (childIdSet[e.source] && childIdSet[e.target]) {
                elems.push({
                    group: 'edges',
                    data: {
                        id: 'ie-' + e.source + '-' + e.target,
                        source: e.source,
                        target: e.target,
                    }
                });
            }
        });
    }

    return elems;
}

/* ---- Ghost nodes and cross-crate edges (added after layout) ---- */
function addCrossCrateElements(crateId) {
    var crateData = state.data.crates[crateId];
    if (!crateData || !crateData.cross_edges || crateData.cross_edges.length === 0) return;

    var cy = state.cy;
    var addedGhostIds = {};

    crateData.cross_edges.forEach(function (ce) {
        var ghostId = dashedId(crateId, ce.target_crate);
        if (!addedGhostIds[ghostId]) {
            var ghostLabel = ce.target_crate;
            cy.add({
                group: 'nodes',
                data: {
                    id: ghostId,
                    label: ghostLabel,
                    level: '1',
                    crate: ce.target_crate,
                    ghost: true,
                    doc: '',
                    children: [],
                },
                classes: 'ghost',
            });
            addedGhostIds[ghostId] = true;
        }

        cy.add({
            group: 'edges',
            data: {
                id: 'ce-' + ce.source + '-' + ce.target_crate,
                source: ce.source,
                target: ghostId,
                target_crate: ce.target_crate,
            },
            classes: 'cross-crate',
        });
    });

    // Position ghost nodes to the right of their source nodes
    cy.nodes('.ghost').forEach(function (ghost) {
        var edgeList = ghost.connectedEdges('.cross-crate');
        if (edgeList.length === 0) return;
        var source = edgeList[0].source();
        if (!source || !source.isNode || !source.isNode()) return;
        var srcPos = source.position();
        ghost.position({
            x: srcPos.x + 180,
            y: srcPos.y,
        });
    });
}