/**
 * cytoscape-init.js - Cytoscape initialization and event wiring
 *
 * Creates the Cytoscape instance, applies the stylesheet, and wires
 * mouse hover (node/edge highlighting), click (navigation/sidebar),
 * and keyboard shortcuts (Escape, Backspace/ArrowLeft).
 */
function initCy() {
    state.cy = cytoscape({
        container: $cy,
        style: CY_STYLES,
        layout: { name: 'grid' },
        minZoom: 0.3,
        maxZoom: 5,
        wheelSensitivity: 1.5,
    });

    // Hover highlight: dim non-connected, brighten connected
    state.cy.on('mouseover', 'node', function (evt) {
        var node = evt.target;
        node.style('border-color', '#e94560');
        node.style('border-width', 3);

        var connected = {};
        node.connectedEdges().forEach(function (edge) {
            connected[edge.id()] = true;
        });

        state.cy.edges().forEach(function (edge) {
            if (connected[edge.id()]) {
                edge.style('line-color', '#e94560');
                edge.style('target-arrow-color', '#e94560');
                edge.style('width', 2.5);
                edge.style('opacity', 1);
                edge.style('z-index', 100);
            } else {
                edge.style('opacity', 0.15);
            }
        });
    });
    state.cy.on('mouseout', 'node', function (evt) {
        var node = evt.target;
        var data = node.data();

        if (data.ghost) {
            node.style('border-color', '#555');
            node.style('border-width', 1);
            node.style('border-style', 'dashed');
        } else if (data.level === '0') {
            node.style('border-width', 2);
            var colors = getLayerColor(data.layer || 0);
            node.style('border-color', colors.border);
            node.style('border-style', 'solid');
        } else if (data.level === '1' && data.hasChildren) {
            node.style('border-color', '#4ecdc4');
            node.style('border-width', 2);
            node.style('border-style', 'double');
        } else {
            node.style('border-color', '#1a4a8a');
            node.style('border-width', 1);
            node.style('border-style', 'solid');
        }

        state.cy.edges().forEach(function (edge) {
            if (edge.hasClass('cross-crate')) {
                edge.style('line-color', '#888');
                edge.style('target-arrow-color', '#888');
                edge.style('width', 1.2);
                edge.style('opacity', 0.5);
            } else {
                edge.style('line-color', '#555');
                edge.style('target-arrow-color', '#555');
                edge.style('width', 1.5);
                edge.style('opacity', 1);
            }
            edge.style('z-index', '');
        });
    });

    // Click handler
    state.cy.on('tap', 'node', function (evt) {
        var node = evt.target;
        var data = node.data();
        var level = data.level;

        if (level === '0') {
            navigateTo('crate', data.id);
        } else if (level === '1') {
            if (data.ghost) {
                var targetCrate = data.crate;
                if (targetCrate && state.data.crates[targetCrate]) {
                    navigateTo('crate', targetCrate);
                }
            } else {
                var crateId = null;
                for (var si = state.stack.length - 1; si >= 0; si--) {
                    if (state.stack[si].level === 'crate') {
                        crateId = state.stack[si].id;
                        break;
                    }
                }
                var children = data.children || [];
                if (children.length > 0) {
                    navigateTo('submodule', data.id);
                } else {
                    renderSidebar(data.id, crateId);
                }
            }
        }
    });

    // Edge tooltip
    state.cy.on('mouseover', 'edge', function (evt) {
        var edge = evt.target;
        edge.style('line-color', '#e94560');
        edge.style('target-arrow-color', '#e94560');
        edge.style('width', edge.hasClass('cross-crate') ? 1.8 : 2.5);
        edge.style('z-index', 100);
    });
    state.cy.on('mouseout', 'edge', function (evt) {
        var edge = evt.target;
        edge.style('line-color', edge.hasClass('cross-crate') ? '#888' : '#555');
        edge.style('target-arrow-color', edge.hasClass('cross-crate') ? '#888' : '#555');
        edge.style('width', edge.hasClass('cross-crate') ? 1.2 : 1.5);
        edge.style('z-index', '');
    });
}